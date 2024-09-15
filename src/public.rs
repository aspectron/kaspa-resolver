use crate::imports::*;
use askama::Template;

use axum::{
    body::Body,
    http::{header, HeaderValue, Request, StatusCode},
    response::{IntoResponse, Response},
};

#[derive(Template)]
#[template(path = "public.html", escape = "none")]
struct PublicTemplate {}

pub async fn json_handler(resolver: &Arc<Resolver>, _req: Request<Body>) -> impl IntoResponse {
    let connections = resolver.connections();
    let connections = connections
        .iter()
        // .filter(|c| c.is_delegate())
        .map(Public::from)
        .collect::<Vec<_>>();
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
}

pub async fn status_handler(_resolver: &Arc<Resolver>, _req: Request<Body>) -> impl IntoResponse {
    let index = PublicTemplate {};

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

#[derive(Serialize)]
pub struct Public<'a> {
    pub version: String,
    #[serde(with = "SerHex::<Strict>")]
    pub sid: u64,
    #[serde(with = "SerHex::<Strict>")]
    pub uid: u64,
    pub service: String,
    pub protocol: ProtocolKind,
    pub encoding: EncodingKind,
    pub encryption: TlsKind,
    pub network: &'a NetworkId,
    pub status: &'static str,
    pub peers: u64,
    pub clients: u64,
    pub capacity: u64,
}

impl<'a> From<&'a Arc<Connection>> for Public<'a> {
    fn from(connection: &'a Arc<Connection>) -> Self {
        let delegate = connection.delegate();

        let node = connection.node();
        let uid = node.uid();
        let service = node.service().to_string();
        let protocol = node.params().protocol();
        let encoding = node.params().encoding();
        let encryption = node.params().tls();
        let network = &node.network;
        let status = connection.status();
        let clients = delegate.clients();
        let peers = delegate.peers();
        let (version, sid, capacity) = delegate
            .caps()
            .as_ref()
            .as_ref()
            .map(|caps| (caps.version.clone(), caps.system_id, caps.clients_limit))
            .unwrap_or_else(|| ("n/a".to_string(), 0, 0));

        Self {
            sid,
            uid,
            version,
            service,
            protocol,
            encoding,
            encryption,
            network,
            status,
            clients,
            peers,
            capacity,
        }
    }
}
