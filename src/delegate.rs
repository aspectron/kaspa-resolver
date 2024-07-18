// use crate::imports::*;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Delegate {
    system_id: u64,
    network_node_uid: u64,
}

impl Delegate {
    pub fn new(system_id: u64, network_node_uid: u64) -> Self {
        Self {
            system_id,
            network_node_uid,
        }
    }
}
