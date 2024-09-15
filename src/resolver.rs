use crate::imports::*;

use axum::{
    // extract::Query,
    body::Body,
    extract::Form,
    http::{header, HeaderValue, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use axum::{error_handling::HandleErrorLayer, BoxError};
use std::time::Duration;
use tokio::net::TcpListener;
use tower::{buffer::BufferLayer, limit::RateLimitLayer, ServiceBuilder};
use tower_http::cors::{Any, CorsLayer};

struct Inner {
    args: Arc<Args>,
    http_server: Mutex<Option<(TcpListener, Router)>>,
    kaspa: Arc<Monitor>,
    sparkle: Arc<Monitor>,
    shutdown_ctl: DuplexChannel<()>,
    events: Channel<Events>,
    sessions: Sessions,
}

impl Inner {
    fn new(args: &Arc<Args>) -> Self {
        Self {
            args: args.clone(),
            http_server: Default::default(),
            kaspa: Arc::new(Monitor::new(args, Service::Kaspa)),
            sparkle: Arc::new(Monitor::new(args, Service::Sparkle)),
            shutdown_ctl: DuplexChannel::oneshot(),
            events: Channel::unbounded(),
            sessions: Sessions::new(HttpStatus::sessions(), HttpStatus::ttl()),
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
        let mut router = Router::new();

        let this = self.clone();
        router = router.route(
            "/v2/kaspa/:network/:tls/:protocol/:encoding",
            get(|path| async move { this.get_elected_kaspa(path).await }),
        );

        let this = self.clone();
        router = router.route(
            "/v2/sparkle/:network/:tls/:protocol/:encoding",
            get(|path| async move { this.get_elected_sparkle(path).await }),
        );

        let this = self.clone();
        router = router.route(
            "/status/logout",
            get(|req: Request<Body>| async move { status::logout_handler(&this, req).await }),
        );

        let this = self.clone();
        router = router.route(
            "/status",
            post(|form: Form<HashMap<String, String>>| async move {
                status::status_handler(&this, status::RequestKind::Post(form)).await
            }),
        );

        let this = self.clone();
        router = router.route(
            "/status",
            get(|req: Request<Body>| async move {
                status::status_handler(&this, status::RequestKind::AsHtml(req)).await
            }),
        );

        let this = self.clone();
        router = router.route(
            "/status/json",
            get(|req: Request<Body>| async move { status::json_handler(&this, req).await }),
        );

        if self.args().public() {
            let this = self.clone();
            router = router.route(
                "/",
                get(|req: Request<Body>| async move { public::status_handler(&this, req).await }),
            );

            let this = self.clone();
            router = router.route(
                "/json",
                get(|req: Request<Body>| async move { public::json_handler(&this, req).await }),
            );
        }

        if let Some(rate_limit) = self.args().rate_limit.as_ref() {
            log_success!(
                "Limits",
                "Setting rate limit to: {} requests per {} seconds",
                rate_limit.requests,
                rate_limit.period
            );
            router = router.layer(
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
            );
        } else {
            log_warn!("Limits", "HTTP rate limit is disabled");
        };

        router = router.layer(CorsLayer::new().allow_origin(Any));

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

    pub async fn start(self: &Arc<Self>) -> Result<()> {
        self.inner.kaspa.start().await?;
        self.inner.sparkle.start().await?;

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

        let mut sessions = workflow_core::task::interval(Duration::from_secs(3600));
        let mut update = workflow_core::task::interval(Updates::duration());

        loop {
            select! {

                msg = events.recv().fuse() => {
                    match msg {
                        Ok(event) => {
                            match event {
                                Events::Start => {
                                    if let Err(err) = self.update(true).await {
                                        log_error!("Config", "[startup] {err}");
                                    }
                                },
                                Events::Update => {
                                    if let Err(err) = self.update(false).await {
                                        log_error!("Config", "[update] {err}");
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

                _ = sessions.next().fuse() => {
                    self.inner.sessions.cleanup();
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

    async fn update_nodes(self: &Arc<Self>, mut global_node_list: Vec<Arc<Node>>) -> Result<()> {
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

    async fn update(self: &Arc<Self>, first_update: bool) -> Result<()> {
        if let Some(node_list) = user_config() {
            // load user config
            // occurs only during start
            if first_update {
                self.update_nodes(node_list).await?;
            }
            Ok(())
        } else if self.args().auto_update {
            // auto update global config
            match update_global_config().await {
                Ok(Some(global_node_list)) => {
                    self.update_nodes(global_node_list).await?;
                    Ok(())
                }
                Ok(None) => Ok(()),
                Err(_) if first_update => {
                    // fallback to local config on first update
                    let node_list = load_config()?;
                    self.update_nodes(node_list).await?;
                    Ok(())
                }
                Err(err) => Err(err),
            }
        } else {
            // no auto update, load local config
            let node_list = load_config()?;
            self.update_nodes(node_list).await?;
            Ok(())
        }
    }

    // // respond with a JSON object containing the status of all nodes
    pub fn connections(&self) -> Vec<Arc<Connection>> {
        let kaspa = self.inner.kaspa.to_vec();

        let sparkle = self.inner.sparkle.to_vec();

        kaspa.into_iter().chain(sparkle).collect::<Vec<_>>()
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

    pub fn sessions(&self) -> &Sessions {
        &self.inner.sessions
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
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static(
                    "no-cache, no-store, must-revalidate, proxy-revalidate, max-age=0",
                ),
            ),
            (header::CONNECTION, HeaderValue::from_static("close")),
        ],
        json,
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
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static(
                    "no-cache, no-store, must-revalidate, proxy-revalidate, max-age=0",
                ),
            ),
            (header::CONNECTION, HeaderValue::from_static("close")),
        ],
        "NOT FOUND",
    )
        .into_response()
}
