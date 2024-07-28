use crate::imports::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeConfig {
    // service type
    pub service: Service,
    // public URL for the node connection
    pub address: Option<String>,
    // is TLS enabled (address wss:// or ws:// ?)
    pub tls: bool,
    // protocol+encoding
    #[serde(rename = "transport-type")]
    pub transport_kind: TransportKind,
    // node network id
    pub network: NetworkId,
    // entry is enabled
    pub enable: Option<bool>,
    // domain name (abc.example.com)
    pub fqdn: String,
}

impl From<NodeConfig> for Node {
    fn from(config: NodeConfig) -> Self {
        let NodeConfig {
            service,
            address,
            tls,
            transport_kind,
            network,
            fqdn,
            ..
        } = config;

        let ws_proto = if tls { "wss://" } else { "ws://" };

        let address = address.unwrap_or_else(|| {
            let transport = Transport {
                kind: transport_kind,
                tls,
                template: format!(
                    "{ws_proto}${{fqdn}}/${{service}}/${{network}}/${{protocol}}/${{encoding}}"
                ),
            };
            transport.make_address(&fqdn, &service, &network)
        });

        let tls = address.starts_with("wss://");
        let uid = xxh3_64(address.as_bytes());
        let uid_string = format!("{uid:016x}");
        let network_node_uid = xxh3_64(format!("{fqdn}{network}{tls}").as_bytes());
        let params = PathParams::new(transport_kind, tls.into(), network);

        Self {
            uid,
            uid_string,
            service,
            params,
            fqdn,
            address,
            transport_kind,
            network,
            network_node_uid,
        }
    }
}

// #[derive(Clone, Debug, Serialize, Deserialize)]
#[derive(Clone, Debug)]
pub struct Node {
    // uid of the node connection (hash(address))
    // #[serde(skip)]
    pub uid: u64,
    // #[serde(skip)]
    pub uid_string: String,
    // contains hash(fqdn+network_id)
    // #[serde(skip)]
    pub network_node_uid: u64,
    // #[serde(skip)]
    pub params: PathParams,

    // ~~

    // service type
    pub service: Service,
    // public URL for the node connection
    pub address: String,
    // protocol+encoding
    // #[serde(rename = "transport-type")]
    pub transport_kind: TransportKind,
    // node network id
    pub network: NetworkId,
    // domain name (abc.example.com)
    pub fqdn: String,
}

impl Eq for Node {}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.uid == other.uid
    }
}

impl std::fmt::Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:016x}] {}", self.uid, self.address)
    }
}

impl Node {
    pub fn new<S1, S2>(
        service: &Service,
        network: NetworkId,
        transport: &Transport,
        fqdn: S1,
        address: S2,
    ) -> Arc<Self>
    where
        S1: Display,
        S2: Display,
    {
        let Transport { tls, kind, .. } = transport;

        let address = address.to_string();
        let fqdn = fqdn.to_string();
        let uid = xxh3_64(address.as_bytes());
        let uid_string = format!("{uid:016x}");

        let network_node_uid = xxh3_64(format!("{fqdn}{network}{tls}").as_bytes());

        let params = PathParams::new(transport.kind, transport.tls.into(), network);

        let node = Self {
            uid,
            uid_string,
            service: *service,
            params,
            fqdn,
            address,
            transport_kind: *kind,
            network,
            network_node_uid,
        };

        Arc::new(node)
    }

    #[inline]
    pub fn params(&self) -> &PathParams {
        &self.params
    }

    #[inline]
    pub fn service(&self) -> Service {
        self.service
    }

    // #[inline]
    // pub fn network(&self) -> NetworkId {
    //     self.network
    // }

    #[inline]
    pub fn uid(&self) -> u64 {
        self.uid
    }

    #[inline]
    pub fn transport_kind(&self) -> TransportKind {
        self.transport_kind
    }

    #[inline]
    pub fn network_node_uid(&self) -> u64 {
        self.network_node_uid
    }

    #[inline]
    pub fn uid_as_str(&self) -> &str {
        self.uid_string.as_str()
    }

    #[inline]
    pub fn address(&self) -> &str {
        self.address.as_str()
    }
}

impl AsRef<Node> for Node {
    fn as_ref(&self) -> &Node {
        self
    }
}
