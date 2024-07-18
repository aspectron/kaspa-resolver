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
    http_server: Mutex<Option<(TcpListener, Router)>>,
    nodes: Mutex<Vec<Arc<NodeConfig>>>,
    kaspa: Arc<Monitor<rpc::kaspa::Client>>,
    sparkle: Arc<Monitor<rpc::sparkle::Client>>,
}

impl Inner {
    fn new(nodes: Vec<Arc<NodeConfig>>) -> Self {
        Self {
            http_server: Default::default(),
            nodes: Mutex::new(nodes),
            kaspa: Arc::new(Monitor::new()),
            sparkle: Arc::new(Monitor::new()),
        }
    }
}

#[derive(Clone)]
pub struct Resolver {
    inner: Arc<Inner>,
}

impl Resolver {
    pub fn try_new(nodes: Vec<Arc<NodeConfig>>) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(Inner::new(nodes)),
        })
    }

    pub async fn init_http_server(self: &Arc<Self>, args: &Args) -> Result<()> {
        let router = Router::new();

        let this = self.clone();
        let router = router.route(
            "/v2/kaspa/wrpc/:tls/:encoding/:network",
            get(|path| async move { this.get_elected_kaspa(path).await }),
            // get(|query, path| async move { this.get_elected_kaspa(query, path).await }),
        );

        let this = self.clone();
        let router = router.route(
            "/v2/sparkle/wrpc/:tls/:encoding/:network",
            get(|path| async move { this.get_elected_sparkle(path).await }),
            // get(|query, path| async move { this.get_elected_sparkle(query, path).await }),
        );

        let router = if args.status {
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

        let router = if let Some(rate_limit) = args.rate_limit.as_ref() {
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

        log_success!("Server", "Listening on http://{}", args.listen.as_str());
        let listener = tokio::net::TcpListener::bind(args.listen.as_str())
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

    pub fn nodes(&self) -> Vec<Arc<NodeConfig>> {
        self.inner.nodes.lock().unwrap().clone()
    }

    pub async fn start(self: &Arc<Self>) -> Result<()> {
        let mut nodes = self.nodes();
        self.inner.kaspa.start(&mut nodes).await?;
        self.inner.sparkle.start(&mut nodes).await?;
        Ok(())
    }

    pub async fn stop(self: &Arc<Self>) -> Result<()> {
        self.inner.sparkle.stop().await?;
        self.inner.kaspa.stop().await?;
        Ok(())
    }

    // respond with a JSON object containing the status of all nodes
    async fn get_status_all_nodes(&self) -> impl IntoResponse {
        let kaspa = self.inner.kaspa.get_all();

        let sparkle = self.inner.sparkle.get_all();

        let status = kaspa
            .iter()
            .map(Output::from)
            .chain(sparkle.iter().map(Output::from))
            .collect::<Vec<_>>();

        with_json(status)
    }

    async fn get_status<T>(&self, monitor: &Monitor<T>) -> impl IntoResponse
    where
        T: rpc::Client + Send + Sync + 'static,
    {
        let connections = monitor.get_all();
        let status = connections.iter().map(Output::from).collect::<Vec<_>>();

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
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::APPLICATION_JSON.as_ref()),
        )],
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
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::APPLICATION_JSON.as_ref()),
        )],
        serde_json::to_string(&data).unwrap(),
    )
        .into_response()
}

#[inline]
#[allow(dead_code)]
fn with_mime(body: impl Into<String>, mime: &'static str) -> Response<Body> {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, HeaderValue::from_static(mime))],
        body.into(),
    )
        .into_response()
}

#[inline]
fn not_found() -> Response<Body> {
    (
        StatusCode::NOT_FOUND,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::TEXT_PLAIN_UTF_8.as_ref()),
        )],
        "NOT FOUND",
    )
        .into_response()
}
