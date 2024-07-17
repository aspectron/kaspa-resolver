use crate::imports::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    #[serde(skip)]
    pub id: u64,
    #[serde(skip)]
    pub id_string: String,

    pub service: Service,
    pub address: String,
    #[serde(rename = "transport-type")]
    pub transport_kind: TransportKind,
    pub tls: bool,
    pub network: NetworkId,
    pub enable: Option<bool>,

    pub fqdn: String,
    // contains hash(fqdn+network_id)
    network_node_uid: u64,
    // pub delegate: Option<Arc<Node>>,
}

impl Eq for Node {}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address
    }
}

impl std::fmt::Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let title = self.address.to_string();
        write!(f, "{}", title)
    }
}

impl Node {
    pub fn new<S1, S2>(
        service: &Service,
        network: NetworkId,
        transport: &Transport,
        fqdn: S1,
        address: S2,
        // delegate : Option<Arc<Node>>,
    ) -> Arc<Self>
    where
        S1: Display,
        S2: Display,
    {
        let address = address.to_string();
        let fqdn = fqdn.to_string();
        let id = xxh3_64(address.as_bytes());
        let id_string = format!("{id:x}");

        let network_node_uid = xxh3_64(format!("{fqdn}{network}").as_bytes());

        let Transport { tls, kind, .. } = transport;

        let node = Self {
            id,
            id_string,
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

    pub fn params(&self) -> PathParams {
        PathParams::new(self.transport_kind, self.tls, self.network)
    }

    pub fn service(&self) -> Service {
        self.service
    }

    pub fn network_node_uid(&self) -> u64 {
        self.network_node_uid
    }
}

impl AsRef<Node> for Node {
    fn as_ref(&self) -> &Node {
        self
    }
}
