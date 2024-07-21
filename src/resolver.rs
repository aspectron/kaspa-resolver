use crate::imports::*;

use axum::{
    // extract::Query,
    body::Body,
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use tokio::net::TcpListener;

use axum::{error_handling::HandleErrorLayer, BoxError};
use std::time::Duration;
use tower::{buffer::BufferLayer, limit::RateLimitLayer, ServiceBuilder};
use tower_http::cors::{Any, CorsLayer};

struct Inner {
    args: Arc<Args>,
    http_server: Mutex<Option<(TcpListener, Router)>>,
    kaspa: Arc<Monitor<rpc::kaspa::Client>>,
    sparkle: Arc<Monitor<rpc::sparkle::Client>>,
    shutdown_ctl: DuplexChannel<()>,
    events: Channel<Events>,
}

impl Inner {
    fn new(args: &Arc<Args>) -> Self {
        Self {
            args: args.clone(),
            http_server: Default::default(),
            kaspa: Arc::new(Monitor::new(args)),
            sparkle: Arc::new(Monitor::new(args)),
            shutdown_ctl: DuplexChannel::oneshot(),
            events: Channel::unbounded(),
        }
    }
}

#[derive(Clone)]
pub struct Resolver {
    inner: Arc<Inner>,
}

impl Resolver {
    pub fn try_new(args: &Arc<Args>) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(Inner::new(args)),
        })
    }

    pub fn args(&self) -> &Arc<Args> {
        &self.inner.args
    }

    pub async fn init_http_server(self: &Arc<Self>) -> Result<()> {
        let router = Router::new();

        let this = self.clone();
        let router = router.route(
            // "/v2/kaspa/wrpc/:tls/:encoding/:network",
            "/v2/kaspa/:network/:tls/:protocol/:encoding",
            get(|path| async move { this.get_elected_kaspa(path).await }),
            // get(|query, path| async move { this.get_elected_kaspa(query, path).await }),
        );

        let this = self.clone();
        let router = router.route(
            "/v2/sparkle/:network/:tls/:protocol/:encoding",
            // "/v2/sparkle/wrpc/:tls/:encoding/:network",
            get(|path| async move { this.get_elected_sparkle(path).await }),
            // get(|query, path| async move { this.get_elected_sparkle(query, path).await }),
        );

        let router = if self.args().status {
            log_warn!("Routes", "Enabling `/status` route");
            let this1 = self.clone();
            let this2 = self.clone();
            let this3 = self.clone();
            router
                .route(
                    "/status",
                    get(|| async move { this1.get_status_all_nodes().await }),
                )
                .route(
                    "/status/kaspa",
                    get(|| async move { this2.get_status(&this2.inner.kaspa).await }),
                )
                .route(
                    "/status/sparkle",
                    get(|| async move { this3.get_status(&this3.inner.sparkle).await }),
                )
        } else {
            log_success!("Routes", "Disabling status routes");
            router
        };

        let router = if let Some(rate_limit) = self.args().rate_limit.as_ref() {
            log_success!(
                "Limits",
                "Setting rate limit to: {} requests per {} seconds",
                rate_limit.requests,
                rate_limit.period
            );
            router.layer(
                ServiceBuilder::new()
                    .layer(HandleErrorLayer::new(|err: BoxError| async move {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("Unhandled error: {}", err),
                        )
                    }))
                    .layer(BufferLayer::new(1024))
                    .layer(RateLimitLayer::new(
                        rate_limit.requests,
                        Duration::from_secs(rate_limit.period),
                    )),
            )
        } else {
            log_warn!("Limits", "Rate limit is disabled");
            router
        };

        let router = router.layer(CorsLayer::new().allow_origin(Any));

        log_success!(
            "Server",
            "Listening on http://{}",
            self.args().listen.as_str()
        );
        let listener = tokio::net::TcpListener::bind(self.args().listen.as_str())
            .await
            .unwrap();

        self.inner
            .http_server
            .lock()
            .unwrap()
            .replace((listener, router));

        Ok(())
    }

    pub async fn listen(self: &Arc<Self>) -> Result<()> {
        let (listener, router) = self.inner.http_server.lock().unwrap().take().unwrap();
        axum::serve(listener, router).await?;
        Ok(())
    }

    // pub fn nodes(&self) -> Vec<Arc<NodeConfig>> {
    //     self.inner.nodes.lock().unwrap().clone()
    // }

    pub async fn start(self: &Arc<Self>) -> Result<()> {
        // let mut nodes = self.nodes();
        // let mut nodes = load_config()?;

        self.inner.kaspa.start().await?;
        self.inner.sparkle.start().await?;
        // let mut nodes = load_config()?;

        // self.inner.kaspa.start(&mut nodes).await?;
        // self.inner.sparkle.start(&mut nodes).await?;

        let this = self.clone();
        spawn(async move {
            if let Err(error) = this.task().await {
                println!("Resolver task error: {:?}", error);
            }
        });

        self.inner.events.send(Events::Start).await?;

        Ok(())
    }

    pub async fn stop(self: &Arc<Self>) -> Result<()> {
        self.inner.sparkle.stop().await?;
        self.inner.kaspa.stop().await?;

        self.inner
            .shutdown_ctl
            .signal(())
            .await
            .expect("Monitor shutdown signal error");

        Ok(())
    }

    async fn task(self: Arc<Self>) -> Result<()> {
        let events = self.inner.events.receiver.clone();
        let shutdown_ctl_receiver = self.inner.shutdown_ctl.request.receiver.clone();
        let shutdown_ctl_sender = self.inner.shutdown_ctl.response.sender.clone();

        let mut update = workflow_core::task::interval(Updates::duration());

        loop {
            select! {

                msg = events.recv().fuse() => {
                    match msg {
                        Ok(event) => {
                            match event {
                                Events::Start => {
                                    if let Err(err) = self.update(true).await {
                                        log_error!("Config", "{err}");
                                    }
                                },
                                Events::Update => {
                                    if let Err(err) = self.update(false).await {
                                        log_error!("Config", "{err}");
                                    }
                                },
                            }
                        }
                        Err(err) => {
                            println!("Monitor: error while receiving events message: {err}");
                            break;
                        }

                    }
                }

                _ = update.next().fuse() => {
                    self.inner.events.send(Events::Update).await?;
                }

                _ = shutdown_ctl_receiver.recv().fuse() => {
                    break;
                },

            }
        }

        shutdown_ctl_sender.send(()).await.unwrap();

        Ok(())
    }

    async fn update_nodes(
        self: &Arc<Self>,
        mut global_node_list: Vec<Arc<NodeConfig>>,
    ) -> Result<()> {
        self.inner.kaspa.update_nodes(&mut global_node_list).await?;
        self.inner
            .sparkle
            .update_nodes(&mut global_node_list)
            .await?;

        for node in global_node_list.iter() {
            log_error!("Update", "Dangling node record: {}", node);
        }
        Ok(())
    }

    async fn update(self: &Arc<Self>, fallback_to_local: bool) -> Result<()> {
        match update_global_config().await {
            Ok(Some(global_node_list)) => {
                self.update_nodes(global_node_list).await?;
                Ok(())
            }
            Ok(None) => Ok(()),
            Err(_) if fallback_to_local => {
                let node_list = load_config()?;
                self.update_nodes(node_list).await?;
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    // async fn update(self: &Arc<Self>) -> Result<()> {
    //     let global_node_list = update_global_config().await?;
    //     self.update_nodes(global_node_list).await?;
    //     Ok(())
    // }

    // respond with a JSON object containing the status of all nodes
    async fn get_status_all_nodes(&self) -> impl IntoResponse {
        let kaspa = self.inner.kaspa.get_all();

        let sparkle = self.inner.sparkle.get_all();

        let status = kaspa
            .iter()
            .map(Status::from)
            .chain(sparkle.iter().map(Status::from))
            .collect::<Vec<_>>();

        with_json(status)
    }

    async fn get_status<T>(&self, monitor: &Monitor<T>) -> impl IntoResponse
    where
        T: rpc::Client + Send + Sync + 'static,
    {
        let connections = monitor.get_all();
        let status = connections.iter().map(Status::from).collect::<Vec<_>>();

        with_json(status)
    }

    // respond with a JSON object containing the elected node
    async fn get_elected_kaspa(
        &self,
        // Query(_query): Query<QueryParams>,
        UrlPath(params): UrlPath<PathParams>,
    ) -> impl IntoResponse {
        // println!("params: {:?}", params);
        // println!("query: {:?}", query);

        if let Some(json) = self.inner.kaspa.election(&params) {
            with_json_string(json)
        } else {
            not_found()
        }
    }

    #[allow(dead_code)]
    async fn get_elected_sparkle(
        &self,
        // Query(_query): Query<QueryParams>,
        UrlPath(params): UrlPath<PathParams>,
    ) -> impl IntoResponse {
        // println!("params: {:?}", params);
        // println!("query: {:?}", query);

        if let Some(json) = self.inner.sparkle.election(&params) {
            with_json_string(json)
        } else {
            not_found()
        }
    }
}

#[inline]
fn with_json_string(json: String) -> Response<Body> {
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static(mime::APPLICATION_JSON.as_ref()),
            ),
            (header::CONNECTION, HeaderValue::from_static("close")),
        ],
        json,
    )
        .into_response()
}

#[inline]
fn with_json<T>(data: T) -> Response<Body>
where
    T: Serialize,
{
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static(mime::APPLICATION_JSON.as_ref()),
            ),
            (header::CONNECTION, HeaderValue::from_static("close")),
        ],
        serde_json::to_string(&data).unwrap(),
    )
        .into_response()
}

#[inline]
#[allow(dead_code)]
fn with_mime(body: impl Into<String>, mime: &'static str) -> Response<Body> {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, HeaderValue::from_static(mime)),
            (header::CONNECTION, HeaderValue::from_static("close")),
        ],
        body.into(),
    )
        .into_response()
}

#[inline]
fn not_found() -> Response<Body> {
    (
        StatusCode::NOT_FOUND,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static(mime::TEXT_PLAIN_UTF_8.as_ref()),
            ),
            (header::CONNECTION, HeaderValue::from_static("close")),
        ],
        "NOT FOUND",
    )
        .into_response()
}
