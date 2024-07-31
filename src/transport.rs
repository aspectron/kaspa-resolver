use crate::imports::*;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum TlsKind {
    Tls,
    None,
    Any,
}

impl Display for TlsKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            TlsKind::Tls => "tls",
            TlsKind::None => "none",
            TlsKind::Any => "any",
        };
        f.write_str(s)
    }
}

impl From<bool> for TlsKind {
    fn from(b: bool) -> Self {
        if b {
            TlsKind::Tls
        } else {
            TlsKind::None
        }
    }
}

#[derive(
    Debug, Describe, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum ProtocolKind {
    Wrpc,
    Grpc,
}

impl Display for ProtocolKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ProtocolKind::Wrpc => "wrpc",
            ProtocolKind::Grpc => "grpc",
        };
        f.write_str(s)
    }
}

#[derive(
    Debug, Describe, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum EncodingKind {
    Borsh,
    Json,
    Protobuf,
}

impl Display for EncodingKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            EncodingKind::Borsh => "borsh",
            EncodingKind::Json => "json",
            EncodingKind::Protobuf => "protobuf",
        };
        f.write_str(s)
    }
}

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
    pub fn protocol(&self) -> ProtocolKind {
        match self {
            TransportKind::WrpcBorsh => ProtocolKind::Wrpc,
            TransportKind::WrpcJson => ProtocolKind::Wrpc,
            TransportKind::Grpc => ProtocolKind::Grpc,
        }
    }

    pub fn encoding(&self) -> EncodingKind {
        match self {
            TransportKind::WrpcBorsh => EncodingKind::Borsh,
            TransportKind::WrpcJson => EncodingKind::Json,
            TransportKind::Grpc => EncodingKind::Protobuf,
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
        // let fqdn = format!("{}/$*/", fqdn);
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

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct TransportDictionary(HashMap<String, Transport>);

impl TransportDictionary {
    pub fn get(&self, key: &str) -> Option<&Transport> {
        self.0.get(key)
    }
}
