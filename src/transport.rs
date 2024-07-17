use crate::imports::*;

#[derive(
    Debug, Describe, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum TransportKind {
    WrpcBorsh,
    WrpcJson,
    Grpc,
}

impl Display for TransportKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            TransportKind::WrpcBorsh => "wrpc-borsh",
            TransportKind::WrpcJson => "wrpc-json",
            TransportKind::Grpc => "grpc",
        };
        f.write_str(s)
    }
}

impl TransportKind {
    pub fn encoding(&self) -> &'static str {
        match self {
            TransportKind::WrpcBorsh => "borsh",
            TransportKind::WrpcJson => "json",
            TransportKind::Grpc => "grpc",
        }
    }

    pub fn protocol(&self) -> &'static str {
        match self {
            TransportKind::WrpcBorsh => "wrpc",
            TransportKind::WrpcJson => "wrpc",
            TransportKind::Grpc => "grpc",
        }
    }

    pub fn wrpc_encoding(&self) -> Option<WrpcEncoding> {
        match self {
            TransportKind::WrpcBorsh => Some(WrpcEncoding::Borsh),
            TransportKind::WrpcJson => Some(WrpcEncoding::SerdeJson),
            TransportKind::Grpc => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransportTemplate {
    #[serde(rename = "type")]
    pub kind: Vec<TransportKind>,
    pub tls: bool,
    pub template: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transport {
    #[serde(rename = "type")]
    pub kind: TransportKind,
    pub tls: bool,
    pub template: String,
}

impl Transport {
    pub fn make_address(&self, fqdn: &str, service: &Service, network_id: &NetworkId) -> String {
        let tpl: Tpl = [
            ("service", service.to_string()),
            ("fqdn", fqdn.to_string()),
            ("network", network_id.to_string()),
            ("protocol", self.kind.protocol().to_string()),
            ("encoding", self.kind.encoding().to_string()),
        ]
        .as_ref()
        .into();

        tpl.render(&self.template)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransportDictionary(HashMap<String, Transport>);

impl TransportDictionary {
    pub fn get(&self, key: &str) -> Option<&Transport> {
        self.0.get(key)
    }
}
