use crate::imports::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeConfig {
    // uid of the node connection (hash(address))
    #[serde(skip)]
    pub uid: u64,
    #[serde(skip)]
    pub uid_string: String,

    // service type
    pub service: Service,
    // public URL for the node connection
    pub address: String,
    // protocol+encoding
    #[serde(rename = "transport-type")]
    pub transport_kind: TransportKind,
    // does this configuration represents a tls connection
    pub tls: bool,
    // node network id
    pub network: NetworkId,
    // entry is enabled
    pub enable: Option<bool>,
    // domain name (abc.example.com)
    pub fqdn: String,
    // contains hash(fqdn+network_id)
    pub network_node_uid: u64,
}

impl Eq for NodeConfig {}

impl PartialEq for NodeConfig {
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address
    }
}

impl std::fmt::Display for NodeConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let title = self.address.to_string();
        write!(f, "{}", title)
    }
}

impl NodeConfig {
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
        let uid_string = format!("{uid:x}");

        let network_node_uid = xxh3_64(format!("{fqdn}{network}{tls}").as_bytes());

        let node = Self {
            uid,
            uid_string,
            service: *service,
            tls: *tls,
            fqdn,
            address,
            transport_kind: *kind,
            network,
            enable: None,
            network_node_uid,
        };

        Arc::new(node)
    }

    #[inline]
    pub fn params(&self) -> PathParams {
        PathParams::new(self.transport_kind, self.tls, self.network)
    }

    #[inline]
    pub fn service(&self) -> Service {
        self.service
    }

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

impl AsRef<NodeConfig> for NodeConfig {
    fn as_ref(&self) -> &NodeConfig {
        self
    }
}
