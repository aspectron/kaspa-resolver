use crate::imports::*;

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize)]
pub struct Delegate {
    #[serde(with = "SerHex::<Compact>")]
    system_id: u64,
    // #[serde(with = "SerHex::<Compact>")]
    // network_node_uid: u64,
    network_id : NetworkId,
}

impl Delegate {
    // pub fn new(system_id: u64, network_node_uid: u64) -> Self {
    pub fn new(system_id: u64, network_id : NetworkId) -> Self {
        Self {
            system_id,
            network_id,
        }
    }
}

impl Display for Delegate {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}:{}", self.system_id, self.network_id)
        // write!(f, "{:016x}:{:016x}", self.system_id, self.network_node_uid)
    }
}