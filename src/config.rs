// #![allow(dead_code)]

// use kaspa_consensus_core::network;

use crate::imports::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    // #[serde(rename = "distribution")]
    // distributions: HashMap<String, HashMap<NetworkId, Distribution>>,
    #[serde(rename = "transport")]
    transports: TransportDictionary,
    // transports: HashMap<String, Transport>,
    #[serde(rename = "group")]
    groups: Option<Vec<Group>>,
    #[serde(rename = "node")]
    nodes: Option<Vec<Node>>,
}

impl Config {
    pub fn try_parse(toml: &str) -> Result<Vec<Arc<Node>>> {
        let config = toml::from_str::<Config>(toml)?;

        let mut nodes: Vec<Arc<Node>> = config
            .nodes
            .map(|nodes| {
                nodes
                    .into_iter()
                    .filter_map(|mut node| {
                        let id = xxh3_64(node.address.as_bytes());
                        let id_string = format!("{id:x}");
                        node.id = id;
                        node.id_string = id_string.chars().take(8).collect();
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
                                    let node =
                                        Node::new(service, *network_id, transport, fqdn, address);
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

pub fn config_folder() -> PathBuf {
    dirs::home_dir()
        .expect("Could not find home folder")
        .join(".kaspa-resolver")
}

pub fn load_config() -> Result<Vec<Arc<Node>>> {
    let config_folder = config_folder();
    let toml = fs::read_to_string(config_folder.join("Resolver.toml"))?;
    Config::try_parse(&toml)
    // let toml = chacha20poly1305::decrypt_slice(&data, &key)?;
    // crate::node::try_parse_nodes(toml.as_str()?)
}

pub fn test_config() -> Result<Vec<Arc<Node>>> {
    let local = include_str!("../Resolver.toml");
    // let config_folder = config_folder();
    // let toml = fs::read_to_string(config_folder.join("Resolver.toml"))?;
    Config::try_parse(local)
}

pub async fn update() -> Result<()> {
    Ok(())
    // let config_folder = config_folder();
    // let key = load_key()?;
    // let data = fs::read(config_folder.join("resolver.bin"))?;
    // let toml = chacha20poly1305::decrypt_slice(&data, &key)?;
    // crate::node::try_parse_nodes(toml.as_str()?)
}

// fn transform(list: Vec<String>) -> Vec<Arc<Node>> {
//     let mut nodes = vec![];
//     for fqdn in list.into_iter() {
//         if fqdn.contains('*') {
//             for n in 0..M_NODES_PER_FQDN {
//                 nodes.push((NetworkId::from_str("mainnet").unwrap(),fqdn.replace('*', &format!("n{n}"))));
//             }
//             for n in 0..T_NODES_PER_FQDN {
//                 nodes.push((NetworkId::from_str("testnet-10").unwrap(),fqdn.replace('*', &format!("a{n}"))));
//                 nodes.push((NetworkId::from_str("testnet-11").unwrap(),fqdn.replace('*', &format!("b{n}"))));
//             }
//         } else {
//             panic!("Invalid FQDN: {}", fqdn);
//         }
//     }

//     nodes.into_iter().map(Node::fqdn).collect()
// }
