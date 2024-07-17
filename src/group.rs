use crate::imports::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Group {
    pub enable: Option<bool>,
    pub fqdn: String,
    pub transports: Vec<String>,
    pub services: Vec<Service>,
    pub network: HashMap<NetworkId, Vec<String>>,
}

impl std::fmt::Display for Group {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.fqdn)
    }
}
