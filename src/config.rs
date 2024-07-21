use crate::imports::*;
use chrono::prelude::*;

const VERSION: u64 = 1;

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

pub fn init() -> Result<()> {
    Settings::load();

    let global_config_folder = global_config_folder();
    if !global_config_folder.exists() {
        fs::create_dir_all(&global_config_folder)?;
    }

    Ok(())
}

pub fn global_config_folder() -> PathBuf {
    dirs::home_dir()
        .expect("Could not find home folder")
        .join(".kaspa-resolver")
}

pub fn local_config_folder() -> Option<PathBuf> {
    let path = std::env::current_exe().ok()?;
    let path = path.parent()?;

    let config_file = path.join("data").join(".data");
    config_file
        .exists()
        .then_some(config_file.parent().unwrap().to_path_buf())
        .or_else(|| {
            path.parent().and_then(|p| p.parent()).and_then(|path| {
                let config_file = path.join("data").join(".data");
                config_file
                    .exists()
                    .then_some(config_file.parent().unwrap().to_path_buf())
            })
        })
}

fn key_file() -> String {
    ".key".to_string()
}

fn global_config_file() -> String {
    format!("resolver.{VERSION}.bin")
}

fn local_config_file() -> String {
    format!("resolver.{VERSION}.toml")
}

fn load_key() -> Result<Secret> {
    Ok(Secret::from(fs::read(
        global_config_folder().join(key_file()),
    )?))
}

pub fn locate_local_config() -> Option<PathBuf> {
    let local_config_file = local_config_file();

    let path = std::env::current_exe().ok()?;
    let path = path.parent()?;

    let config_file = path.join(&local_config_file);
    config_file.exists().then_some(config_file).or_else(|| {
        path.parent().and_then(|p| p.parent()).and_then(|path| {
            let config_file = path.join("data").join(&local_config_file);
            config_file.exists().then_some(config_file)
        })
    })
}

pub fn test_config() -> Result<Vec<Arc<NodeConfig>>> {
    let local_config = locate_local_config().ok_or(Error::LocalConfigNotFound)?;
    let toml = fs::read_to_string(local_config)?;
    // let local = include_str!("../Resolver.toml");
    Config::try_parse(toml.as_str())
}

pub fn load_config() -> Result<Vec<Arc<NodeConfig>>> {
    match load_global_config() {
        Ok(config) => Ok(config),
        Err(_) => load_default_config(),
    }
}

pub fn load_global_config() -> Result<Vec<Arc<NodeConfig>>> {
    let global_config_folder = global_config_folder();
    if !global_config_folder.exists() {
        fs::create_dir_all(&global_config_folder)?;
    }
    let key = load_key()?;
    let data = fs::read(global_config_folder.join(global_config_file()))?;
    let toml = chacha20poly1305::decrypt_slice(&data, &key)?;
    Config::try_parse(toml.as_str()?)
}

pub fn load_default_config() -> Result<Vec<Arc<NodeConfig>>> {
    let local_config_folder = local_config_folder().ok_or(Error::LocalConfigNotFound)?;
    let local_config = local_config_folder.join(local_config_file());
    let toml = fs::read_to_string(local_config)?;
    Config::try_parse(toml.as_str())
}

pub async fn update_global_config() -> Result<Option<Vec<Arc<NodeConfig>>>> {
    static HASH: Mutex<Option<Vec<u8>>> = Mutex::new(None);

    let url = format!("{}{}", Updates::url(), global_config_file());
    let data = reqwest::get(&url).await?.bytes().await?.to_vec();

    if data.len() < 24 {
        println!("Error fetching: {url}");
        return Err(Error::custom(format!(
            "Update: invalid data length: {}",
            data.len()
        )));
    }

    let hash = sha256(data.as_slice());
    let mut previous = HASH.lock().unwrap();
    if previous.as_deref() == Some(hash.as_bytes()) {
        Ok(None)
    } else {
        *previous = Some(hash.as_bytes().to_vec());
        let key = load_key()?;
        let toml = chacha20poly1305::decrypt_slice(&data, &key)?;
        let config = Config::try_parse(toml.as_str()?)?;
        fs::write(global_config_folder().join(global_config_file()), data)?;
        Ok(Some(config))
    }
}

pub fn generate_key() -> Result<()> {
    let key_path = global_config_folder().join(key_file());
    if key_path.exists() {
        if let Ok(key) = fs::read(&key_path) {
            if key.len() != 32 {
                log::error("Detected a key file with invalid length... overwriting...")?;
            } else {
                let prefix = u16::from_be_bytes(key.as_slice()[0..2].try_into().unwrap());
                if !cliclack::confirm(format!("Found existing key `{prefix:04x}`. Overwrite?"))
                    .interact()?
                {
                    return Ok(());
                }
            }
        } else if !cliclack::confirm("Key already exists. Overwrite?").interact()? {
            return Ok(());
        }
    }

    match cliclack::password("Enter password:").interact() {
        Ok(password1) => match cliclack::password("Enter password:").interact() {
            Ok(password2) => {
                if password1 != password2 {
                    return Err(Error::PasswordsDoNotMatch);
                }
                let key = argon2_sha256(password1.as_bytes(), 32)?;
                let prefix = u16::from_be_bytes(key.as_bytes()[0..2].try_into().unwrap());
                fs::write(key_path, key.as_bytes())?;

                cliclack::outro(format!("Key `{prefix:04x}` generated successfully"))?;
                println!();
            }
            Err(_) => {
                log::error("Failed to read password")?;
            }
        },
        Err(_) => {
            log::error("Failed to read password")?;
        }
    }

    Ok(())
}

pub fn get_key() -> Result<Secret> {
    let key = match load_key() {
        Ok(key) => key,
        Err(_) => {
            generate_key()?;
            load_key()?
        }
    };

    Ok(key)
}

fn prefix(key: &Secret) -> String {
    let prefix = u16::from_be_bytes(key.as_bytes()[0..2].try_into().unwrap());
    format!("{prefix:04x}")
}

pub fn pack() -> Result<()> {
    let key = get_key()?;
    log::info(format!("Packing key prefix `{}`", prefix(&key)))?;
    let local_config_folder = local_config_folder().ok_or(Error::LocalConfigNotFound)?;
    let local_config_file = local_config_folder.join(local_config_file());
    let local_data_file = local_config_folder.join(global_config_file());
    let toml = fs::read_to_string(local_config_file)?;
    Config::try_parse(toml.as_str())?;
    let data = chacha20poly1305::encrypt_slice(toml.as_bytes(), &key)?;
    fs::write(local_data_file, &data)?;
    log::success(format!("Package size {}", data.len()))?;
    outro("Have a great day!")?;
    Ok(())
}

pub fn unpack() -> Result<()> {
    let key = get_key()?;
    log::info(format!("Unpacking key prefix `{}`", prefix(&key)))?;
    let local_config_folder = local_config_folder().ok_or(Error::LocalConfigNotFound)?;
    let local_data_file = local_config_folder.join(global_config_file());
    let local_config_file = if local_config_folder.join(local_config_file()).exists() {
        let local_config_file = local_config_file();
        let now = Local::now();
        let ts = now.format("%Y-%m-%d-%H-%M-%S");
        let local_config_file_ts =
            format!("{}.{}.toml", local_config_file.replace(".toml", ""), ts);
        local_config_folder.join(local_config_file_ts)
    } else {
        local_config_folder.join(local_config_file())
    };
    let data = fs::read(local_data_file)?;
    let toml = chacha20poly1305::decrypt_slice(&data, &key)?;
    Config::try_parse(toml.as_str()?)?;
    fs::write(&local_config_file, toml)?;
    log::success(format!(
        "Unpacked TOML at: `{}`",
        local_config_file.display()
    ))?;
    outro("Have a great day!")?;
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    updates: Updates,
    limits: Limits,
}

impl Settings {
    pub fn load() {
        let _ = Settings::get();
    }

    pub fn get() -> &'static Self {
        static SETTINGS: OnceLock<Settings> = OnceLock::new();
        SETTINGS.get_or_init(|| {
            let toml = include_str!("../Resolver.toml");
            toml::from_str::<Settings>(toml).unwrap()
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Updates {
    pub url: String,
    pub duration: f64,
}

impl Updates {
    pub fn url() -> &'static str {
        Settings::get().updates.url.as_str()
    }

    pub fn duration() -> Duration {
        let seconds = Settings::get().updates.duration * 60.0 * 60.0;
        Duration::from_secs_f64(seconds)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Limits {
    pub fd: u64,
}

impl Limits {
    pub fn fd() -> u64 {
        Settings::get().limits.fd
    }
}
