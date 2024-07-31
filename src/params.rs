use crate::imports::*;

pub static NETWORKS: &[NetworkId] = &[
    NetworkId::new(NetworkType::Mainnet),
    NetworkId::with_suffix(NetworkType::Testnet, 10),
    NetworkId::with_suffix(NetworkType::Testnet, 11),
    // NetworkId::new(NetworkType::Devnet),
    // NetworkId::new(NetworkType::Simnet),
];

pub static TRANSPORTS: &[TransportKind] = &[
    TransportKind::WrpcBorsh,
    TransportKind::WrpcJson,
    // TransportKind::Grpc,
];

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PathParams {
    pub protocol: ProtocolKind,
    pub encoding: EncodingKind,
    pub network: NetworkId,
    pub tls: TlsKind,
}

impl PathParams {
    pub fn new(transport_kind: TransportKind, tls: TlsKind, network: NetworkId) -> Self {
        let protocol = transport_kind.protocol();
        let encoding = transport_kind.encoding();
        Self {
            protocol,
            encoding,
            // transport_kind,
            tls,
            network,
        }
    }

    // iterates only TlsKind::Tls and TlsKind::None variants
    pub fn iter_tls_strict() -> impl Iterator<Item = PathParams> {
        NETWORKS.iter().flat_map(move |network_id| {
            TRANSPORTS
                .iter()
                .map(move |transport_kind| {
                    PathParams::new(*transport_kind, TlsKind::Tls, *network_id)
                })
                .chain(TRANSPORTS.iter().map(move |transport_kind| {
                    PathParams::new(*transport_kind, TlsKind::None, *network_id)
                }))
        })
    }

    // iterates TlsKind::Tls, TlsKind::None, and TlsKind::Any variants
    pub fn iter_tls_any() -> impl Iterator<Item = PathParams> {
        NETWORKS.iter().flat_map(move |network_id| {
            TRANSPORTS
                .iter()
                .map(move |transport_kind| {
                    PathParams::new(*transport_kind, TlsKind::Tls, *network_id)
                })
                .chain(TRANSPORTS.iter().map(move |transport_kind| {
                    PathParams::new(*transport_kind, TlsKind::None, *network_id)
                }))
                .chain(TRANSPORTS.iter().map(move |transport_kind| {
                    PathParams::new(*transport_kind, TlsKind::Any, *network_id)
                }))
        })
    }

    #[inline]
    pub fn protocol(&self) -> ProtocolKind {
        self.protocol
    }

    #[inline]
    pub fn encoding(&self) -> EncodingKind {
        self.encoding
    }

    #[inline]
    pub fn tls(&self) -> TlsKind {
        self.tls
    }

    #[inline]
    pub fn to_tls(self, tls: TlsKind) -> Self {
        Self { tls, ..self }
    }

    #[inline]
    pub fn is_tls_strict(&self) -> bool {
        matches!(self.tls, TlsKind::Tls | TlsKind::None)
    }
}

impl fmt::Display for PathParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}:{}",
            self.tls, self.protocol, self.encoding, self.network
        )
    }
}

// ---

// #[derive(Debug, Deserialize)]
// pub struct QueryParams {
//     // Accessible via a query string like "?access=utxo-index+tx-index+block-dag+metrics+visualizer+mining"
//     #[allow(dead_code)]
//     pub access: Option<AccessList>,
// }

// #[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Hash)]
// #[serde(rename_all = "kebab-case")]
// pub enum AccessType {
//     NoOp,
// }

// impl AccessType {
//     #[allow(dead_code)]
//     pub fn iter() -> impl Iterator<Item = AccessType> {
//         [
//             AccessType::Transact,
//             AccessType::Mempool,
//             AccessType::BlockDag,
//             AccessType::Network,
//             AccessType::Metrics,
//             AccessType::Visualizer,
//             AccessType::Mining,
//         ]
//         .into_iter()
//     }
// }

// impl fmt::Display for AccessType {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         let s = match self {
//             AccessType::Transact => "transact",
//             AccessType::Mempool => "mempool",
//             AccessType::BlockDag => "block-dag",
//             AccessType::Network => "network",
//             AccessType::Metrics => "metrics",
//             AccessType::Visualizer => "visualizer",
//             AccessType::Mining => "mining",
//         };
//         write!(f, "{s}")
//     }
// }

// impl FromStr for AccessType {
//     type Err = String;
//     fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
//         match s {
//             "transact" => Ok(AccessType::Transact),
//             "mempool" => Ok(AccessType::Mempool),
//             "block-dag" => Ok(AccessType::BlockDag),
//             "network" => Ok(AccessType::Network),
//             "metrics" => Ok(AccessType::Metrics),
//             "visualizer" => Ok(AccessType::Visualizer),
//             "mining" => Ok(AccessType::Mining),
//             _ => Err(format!("Invalid access type: {}", s)),
//         }
//     }
// }

// #[derive(Debug, Clone)]
// pub struct AccessList {
//     pub access: Vec<AccessType>,
// }

// impl std::fmt::Display for AccessList {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(
//             f,
//             "{}",
//             self.access
//                 .iter()
//                 .map(|access| access.to_string())
//                 .collect::<Vec<_>>()
//                 .join(" ")
//         )
//     }
// }

// impl FromStr for AccessList {
//     type Err = String;

//     fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
//         let access = s
//             .split(' ')
//             .map(|s| s.parse::<AccessType>())
//             .collect::<std::result::Result<Vec<_>, _>>()?;
//         Ok(AccessList { access })
//     }
// }

// impl Serialize for AccessList {
//     fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
//     where
//         S: Serializer,
//     {
//         serializer.serialize_str(&self.to_string())
//     }
// }

// struct AccessListVisitor;
// impl<'de> de::Visitor<'de> for AccessListVisitor {
//     type Value = AccessList;
//     fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
//         formatter.write_str("a string containing list of permissions separated by a '+'")
//     }

//     fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
//     where
//         E: de::Error,
//     {
//         AccessList::from_str(value).map_err(|err| de::Error::custom(err.to_string()))
//     }
// }

// impl<'de> Deserialize<'de> for AccessList {
//     fn deserialize<D>(deserializer: D) -> std::result::Result<AccessList, D::Error>
//     where
//         D: Deserializer<'de>,
//     {
//         deserializer.deserialize_str(AccessListVisitor)
//     }
// }
