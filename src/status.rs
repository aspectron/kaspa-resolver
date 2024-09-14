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

#[derive(Template)]
#[template(path = "index.html", escape = "none")]
struct IndexTemplate {
    access: bool,
    version : &'static str,
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

            let index = IndexTemplate { access: true, version : crate::VERSION };

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
            let index = IndexTemplate { access: false, version : crate::VERSION };
            NoCacheHtml(index.render().unwrap()).into_response()
        }
        _ => {
            let index = IndexTemplate { access: false, version : crate::VERSION };
            NoCacheHtml(index.render().unwrap()).into_response()
        }
    }
}

#[derive(Serialize)]
pub struct Status<'a> {
    pub version: String,
    #[serde(with = "SerHex::<Strict>")]
    pub sid: u64,
    #[serde(with = "SerHex::<Strict>")]
    pub uid: u64,
    pub url: &'a str,
    pub fqdn: &'a str,
    pub service: String,
    // pub service: &'a str,
    pub protocol: ProtocolKind,
    pub encoding: EncodingKind,
    pub encryption: TlsKind,
    pub network: &'a NetworkId,
    pub cores: u64,
    pub memory: u64,
    pub status: &'static str,
    pub peers: u64,
    pub clients: u64,
    pub capacity: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegates: Option<Vec<String>>,
}

impl<'a> From<&'a Arc<Connection>> for Status<'a> {
    fn from(connection: &'a Arc<Connection>) -> Self {
        let delegate = connection.delegate();

        let node = connection.node();
        let uid = node.uid();
        let url = node.address.as_str();
        let fqdn = node.fqdn.as_str();
        let service = node.service().to_string();
        let protocol = node.params().protocol();
        let encoding = node.params().encoding();
        let encryption = node.params().tls();
        let network = &node.network;
        let status = connection.status();
        let clients = delegate.clients();
        let peers = delegate.peers();
        let (version, sid, capacity, cores, memory) = delegate
            .caps()
            .as_ref()
            .as_ref()
            .map(|caps| {
                (
                    caps.version.clone(),
                    caps.system_id,
                    caps.clients_limit,
                    caps.cpu_physical_cores,
                    caps.total_memory,
                )
            })
            .unwrap_or_else(|| ("n/a".to_string(), 0, 0, 0, 0));

        let delegates = connection
            .resolve_delegators()
            .iter()
            .map(|connection| format!("[{:016x}] {}", connection.system_id(), connection.address()))
            .collect::<Vec<String>>();
        let delegates = (!delegates.is_empty()).then_some(delegates);

        Self {
            sid,
            uid,
            version,
            fqdn,
            service,
            url,
            protocol,
            encoding,
            encryption,
            network,
            cores,
            memory,
            status,
            clients,
            peers,
            capacity,
            delegates,
        }
    }
}
