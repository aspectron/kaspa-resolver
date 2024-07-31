use crate::imports::*;
use askama::Template;

use axum::{
    body::Body,
    extract::Form,
    http::{header, HeaderValue, Request, StatusCode},
    response::{IntoResponse, Redirect, Response},
};

pub enum RequestKind {
    AsHtml(Request<Body>),
    Post(Form<HashMap<String, String>>),
}

#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct NoCacheHtml<T>(pub T);

impl<T> IntoResponse for NoCacheHtml<T>
where
    T: Into<Body>,
{
    fn into_response(self) -> Response {
        (
            [
                (
                    header::CONTENT_TYPE,
                    HeaderValue::from_static(mime::TEXT_HTML_UTF_8.as_ref()),
                ),
                (
                    header::CACHE_CONTROL,
                    HeaderValue::from_static(
                        "no-cache, no-store, must-revalidate, proxy-revalidate, max-age=0",
                    ),
                ),
            ],
            self.0.into(),
        )
            .into_response()
    }
}

impl<T> From<T> for NoCacheHtml<T> {
    fn from(inner: T) -> Self {
        Self(inner)
    }
}

#[derive(Template)]
#[template(path = "index.html", escape = "none")]
struct IndexTemplate {
    access: bool,
}

pub async fn logout_handler(resolver: &Arc<Resolver>, req: Request<Body>) -> impl IntoResponse {
    if let Some(session_id) = session_id_from_req(&req) {
        resolver.sessions().remove(session_id);
    }
    Redirect::to("/status").into_response()
}

fn validate_passphrase(passphrase: &str) -> bool {
    static KEY: OnceLock<Option<u64>> = OnceLock::new();
    let key = KEY.get_or_init(|| load_key64().ok());
    if let Some(key64) = key {
        let pass64 = xxh3_64(passphrase.trim().as_bytes());
        pass64 == *key64
    } else {
        false
    }
}

pub fn session_id_from_req(req: &Request<Body>) -> Option<&str> {
    let cookie_header = req.headers().get("cookie")?;
    let cookie = cookie_header.to_str().unwrap_or("");
    cookie
        .split(';')
        .filter_map(|s| {
            let mut kv = s.split('=');
            (kv.next().map(|s| s.trim()) == Some("session"))
                .then(|| kv.next())
                .flatten()
                .map(|s| s.trim())
        })
        .next()
}

#[inline]
pub fn session_from_req(resolver: &Arc<Resolver>, req: &Request<Body>) -> Option<Session> {
    let session_id = session_id_from_req(req)?;
    resolver.sessions().get(session_id)
}

pub fn resolve_session(
    resolver: &Arc<Resolver>,
    req: &RequestKind,
) -> Result<(Option<Session>, Option<String>)> {
    match req {
        RequestKind::Post(Form(params)) => {
            if let Some(passphrase) = params.get("passphrase") {
                if validate_passphrase(passphrase) {
                    let session_id = uuid::Uuid::new_v4().to_string();
                    let session = Session::default();
                    resolver.sessions().set(&session_id, session.clone());
                    let cookie = format!("session={}; HttpOnly; Secure; SameSite=Lax", session_id);
                    Ok((Some(session), Some(cookie)))
                } else {
                    Err(Error::Unauthorized)
                }
            } else {
                Err(Error::Http(StatusCode::BAD_REQUEST, "Bad request"))
            }
        }
        RequestKind::AsHtml(req) => Ok((session_from_req(resolver, req), None)),
    }
}

pub async fn json_handler(resolver: &Arc<Resolver>, req: Request<Body>) -> impl IntoResponse {
    if session_from_req(resolver, &req).is_some() {
        let connections = resolver.connections(); //.iter().map(Status::from).collect::<Vec<_>>();
        let connections = connections.iter().map(Status::from).collect::<Vec<_>>();
        let nodes = serde_json::to_string(&connections).unwrap();
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .header(
                header::CACHE_CONTROL,
                HeaderValue::from_static(
                    "no-cache, no-store, must-revalidate, proxy-revalidate, max-age=0",
                ),
            )
            .body(Body::from(nodes))
            .unwrap()
    } else {
        Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(header::CONTENT_TYPE, "application/json")
            .header(
                header::CACHE_CONTROL,
                HeaderValue::from_static(
                    "no-cache, no-store, must-revalidate, proxy-revalidate, max-age=0",
                ),
            )
            .body(Body::from(""))
            .unwrap()
    }
}

pub async fn status_handler(resolver: &Arc<Resolver>, req: RequestKind) -> impl IntoResponse {
    let ctx = resolve_session(resolver, &req);
    match ctx {
        Ok((Some(session), cookie)) => {
            session.touch();

            let index = IndexTemplate { access: true };

            if let Some(cookie) = cookie {
                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::SET_COOKIE, cookie)
                    .header(
                        header::CACHE_CONTROL,
                        HeaderValue::from_static(
                            "no-cache, no-store, must-revalidate, proxy-revalidate, max-age=0",
                        ),
                    )
                    .body(Body::from(index.render().unwrap()))
                    .unwrap()
            } else {
                Response::builder()
                    .status(StatusCode::OK)
                    .header(
                        header::CACHE_CONTROL,
                        HeaderValue::from_static(
                            "no-cache, no-store, must-revalidate, proxy-revalidate, max-age=0",
                        ),
                    )
                    .body(Body::from(index.render().unwrap()))
                    .unwrap()
            }
        }
        Err(Error::Http(status, msg)) => Response::builder()
            .status(status)
            .body(Body::from(msg))
            .unwrap(),
        Err(Error::Unauthorized) => {
            let index = IndexTemplate { access: false };
            NoCacheHtml(index.render().unwrap()).into_response()
        }
        _ => {
            let index = IndexTemplate { access: false };
            NoCacheHtml(index.render().unwrap()).into_response()
        }
    }
}
