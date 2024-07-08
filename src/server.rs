// use crate::imports::*;

// use crate::monitor::monitor;
// use axum::{
//     async_trait,
//     extract::{path::ErrorKind, rejection::PathRejection, FromRequestParts, Query},
//     http::{header, request::Parts, HeaderValue, StatusCode},
//     response::IntoResponse,
//     routing::get,
//     // Json,
//     Router,
// };
// use tokio::net::TcpListener;

// use axum::{error_handling::HandleErrorLayer, BoxError};
// use std::time::Duration;
// // use tower::{buffer::BufferLayer, limit::RateLimitLayer, ServiceBuilder};
// use tower_http::cors::{Any, CorsLayer};

// // use crate::result::Result;
// // use crate::error::Error;

// // impl std::str::FromStr for Tls {
// //     type Err = Error;

// //     fn from_str(s: &str) -> Result<Self> {
// //         match s {
// //             "ssl" => Ok(Tls::Ssl),
// //             "none" => Ok(Tls::None),
// //             "any" => Ok(Tls::Any),
// //             _ => Err(Error::custom(format!("Invalid TLS option: {}", s))),
// //         }
// //     }
// // }

// // impl std::fmt::Display for Tls {
// //     fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
// //         match self {
// //             Tls::Ssl => write!(f, "ssl"),
// //             Tls::None => write!(f, "none"),
// //             Tls::Any => write!(f, "any"),
// //         }
// //     }
// // }

// // pub async fn server(args: &Args) -> Result<(TcpListener, Router)> {
// //     // initialize tracing
// //     // tracing_subscriber::fmt::init();

// //     let app = Router::new().route(
// //         "/v2/rk/wrpc/:tls/:encoding/:network",
// //         get(get_elected_kaspad),
// //     );
// //     // let app = app.route("/v2/sparkle/wrpcs/:encoding/:network", get(get_elected_sparkle));
// //     // let app = app.route("/v2/sparkle/wrpc/:encoding/:network", get(get_elected_sparkle));

// //     let app = if args.status {
// //         log_warn!("Routes", "Enabling `/status` route");
// //         app.route("/status", get(get_status_all_nodes))
// //     } else {
// //         log_success!("Routes", "Disabling `/status` route");
// //         app
// //     };

// //     let app = if let Some(rate_limit) = args.rate_limit.as_ref() {
// //         log_success!(
// //             "Limits",
// //             "Setting rate limit to: {} requests per {} seconds",
// //             rate_limit.requests,
// //             rate_limit.period
// //         );
// //         app.layer(
// //             ServiceBuilder::new()
// //                 .layer(HandleErrorLayer::new(|err: BoxError| async move {
// //                     (
// //                         StatusCode::INTERNAL_SERVER_ERROR,
// //                         format!("Unhandled error: {}", err),
// //                     )
// //                 }))
// //                 .layer(BufferLayer::new(1024))
// //                 .layer(RateLimitLayer::new(
// //                     rate_limit.requests,
// //                     Duration::from_secs(rate_limit.period),
// //                 )),
// //         )
// //     } else {
// //         log_warn!("Limits", "Rate limit is disabled");
// //         app
// //     };

// //     let app = app.layer(CorsLayer::new().allow_origin(Any));

// //     log_success!("Server", "Listening on http://{}", args.listen.as_str());
// //     let listener = tokio::net::TcpListener::bind(args.listen.as_str())
// //         .await
// //         .unwrap();
// //     Ok((listener, app))
// // }

// // // respond with a JSON object containing the status of all nodes
// // async fn get_status_all_nodes() -> impl IntoResponse {
// //     let json = monitor().get_all_json();
// //     (
// //         StatusCode::OK,
// //         [(
// //             header::CONTENT_TYPE,
// //             HeaderValue::from_static(mime::APPLICATION_JSON.as_ref()),
// //         )],
// //         json,
// //     )
// //         .into_response()
// // }

// // // respond with a JSON object containing the elected node
// // async fn get_elected_kaspad(
// //     Query(_query): Query<QueryParams>,
// //     UrlPath(params): UrlPath<PathParams>,
// // ) -> impl IntoResponse {
// //     // println!("params: {:?}", params);
// //     // println!("query: {:?}", query);

// //     if let Some(json) = monitor().get_json(&params) {
// //         (
// //             [(
// //                 header::CONTENT_TYPE,
// //                 HeaderValue::from_static(mime::APPLICATION_JSON.as_ref()),
// //             )],
// //             json,
// //         )
// //             .into_response()
// //     } else {
// //         (
// //             StatusCode::NOT_FOUND,
// //             [(
// //                 header::CONTENT_TYPE,
// //                 HeaderValue::from_static(mime::TEXT_PLAIN_UTF_8.as_ref()),
// //             )],
// //             "NOT FOUND".to_string(),
// //         )
// //             .into_response()
// //     }
// // }

// // #[allow(dead_code)]
// // async fn get_elected_sparkle(
// //     Query(_query): Query<QueryParams>,
// //     UrlPath(params): UrlPath<PathParams>,
// // ) -> impl IntoResponse {
// //     // println!("params: {:?}", params);
// //     // println!("query: {:?}", query);

// //     if let Some(json) = monitor().get_json(&params) {
// //         (
// //             [(
// //                 header::CONTENT_TYPE,
// //                 HeaderValue::from_static(mime::APPLICATION_JSON.as_ref()),
// //             )],
// //             json,
// //         )
// //             .into_response()
// //     } else {
// //         (
// //             StatusCode::NOT_FOUND,
// //             [(
// //                 header::CONTENT_TYPE,
// //                 HeaderValue::from_static(mime::TEXT_PLAIN_UTF_8.as_ref()),
// //             )],
// //             "NOT FOUND".to_string(),
// //         )
// //             .into_response()
// //     }
// // }
