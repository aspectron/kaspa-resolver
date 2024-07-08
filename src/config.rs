#![allow(dead_code)]

use crate::imports::*;

pub fn config_folder() -> PathBuf {
    dirs::home_dir()
        .expect("Could not find home folder")
        .join(".kaspa-resolver")
    // ctx.home_folder.join(".kaspa-resolver")
}

fn load_key() -> Result<Secret> {
    let config_folder = config_folder();
    Ok(Secret::from(fs::read(config_folder.join("key"))?))
}

pub fn load_config() -> Result<Vec<Arc<Node>>> {
    let config_folder = config_folder();
    let key = load_key()?;
    let data = fs::read(config_folder.join("resolver.bin"))?;
    let toml = chacha20poly1305::decrypt_slice(&data, &key)?;
    crate::node::try_parse_nodes(toml.as_str()?)
}

pub async fn update() -> Result<()> {
    Ok(())
    // let config_folder = config_folder();
    // let key = load_key()?;
    // let data = fs::read(config_folder.join("resolver.bin"))?;
    // let toml = chacha20poly1305::decrypt_slice(&data, &key)?;
    // crate::node::try_parse_nodes(toml.as_str()?)
}
