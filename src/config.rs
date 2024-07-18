use crate::imports::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(rename = "transport")]
    transports: TransportDictionary,
    #[serde(rename = "group")]
    groups: Option<Vec<Group>>,
    #[serde(rename = "node")]
    nodes: Option<Vec<NodeConfig>>,
}

impl Config {
    pub fn try_parse(toml: &str) -> Result<Vec<Arc<NodeConfig>>> {
        let config = toml::from_str::<Config>(toml)?;

        let mut nodes: Vec<Arc<NodeConfig>> = config
            .nodes
            .map(|nodes| {
                nodes
                    .into_iter()
                    .filter_map(|mut node| {
                        node.uid = xxh3_64(node.address.as_bytes());
                        node.enable.unwrap_or(true).then_some(node).map(Arc::new)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let groups: Vec<Group> = config
            .groups
            .map(|groups| {
                groups
                    .into_iter()
                    .filter_map(|cluster| cluster.enable.unwrap_or(true).then_some(cluster))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut unique_groups = HashSet::new();
        for group in groups.iter() {
            if !unique_groups.insert(group.fqdn.clone()) {
                return Err(Error::config(format!("Duplicate group: {}", group.fqdn)));
            }
        }

        let transport_dictionary = &config.transports;

        for group in groups.iter() {
            if !group.fqdn.contains('*') {
                log_error!("Config", "Invalid group FQDN: {}", group.fqdn);
            } else {
                let Group {
                    fqdn,
                    transports,
                    services,
                    network,
                    ..
                } = group;

                for service in services.iter() {
                    for (network_id, ids) in network.iter() {
                        for transport in transports.iter() {
                            for id in ids {
                                if let Some(transport) = transport_dictionary.get(transport) {
                                    let fqdn = fqdn.replace('*', &id.to_lowercase());
                                    let address =
                                        transport.make_address(&fqdn, service, network_id);
                                    let node = NodeConfig::new(
                                        service,
                                        *network_id,
                                        transport,
                                        fqdn,
                                        address,
                                    );
                                    nodes.push(node);
                                } else {
                                    log_error!("Config", "Unknown transport: {}", transport);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(nodes)
    }
}

#[allow(dead_code)]
pub fn config_folder() -> PathBuf {
    dirs::home_dir()
        .expect("Could not find home folder")
        .join(".kaspa-resolver")
}

pub fn load_config() -> Result<Vec<Arc<NodeConfig>>> {
    // let config_folder = config_folder();
    // let toml = fs::read_to_string(config_folder.join("Resolver.toml"))?;
    let toml = include_str!("../Resolver.toml");
    Config::try_parse(toml)
}

pub fn test_config() -> Result<Vec<Arc<NodeConfig>>> {
    let local = include_str!("../Resolver.toml");
    Config::try_parse(local)
}

pub async fn update() -> Result<()> {
    Ok(())
}
