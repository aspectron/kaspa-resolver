pub mod kaspa;
pub mod sparkle;

use crate::imports::*;

const SOCKETS_PER_CORE: u32 = 768;

#[allow(dead_code)]
#[derive(Debug)]
pub struct Caps {
    // node version
    pub version: String,
    // node system id
    pub system_id: u64,
    // git hash
    pub git_hash: Option<String>,
    // current memory usage in bytes
    pub total_memory: u64,
    // number of cores
    pub cpu_physical_cores: u64,
    // number of available file descriptors
    pub fd_limit: u64,
    // number of available clients
    pub clients_limit: u64,
    // client capacity: min(fd_limit, clients_limit)
    pub capacity: u64,
}

impl Caps {
    pub fn system_id(&self) -> u64 {
        self.system_id
    }
}

#[derive(Debug)]
pub struct Connections {
    pub clients: u64,
    #[allow(dead_code)]
    pub peers: u64,
}

#[enum_dispatch]
#[derive(Debug)]
pub enum Client {
    Kaspa(kaspa::Client),
    Sparkle(sparkle::Client),
}

#[enum_dispatch(Client)]
pub trait ClientT: std::fmt::Debug + Sized + Send + Sync + 'static {
    fn multiplexer(&self) -> Multiplexer<Ctl> {
        unimplemented!()
    }

    async fn connect(&self) -> Result<()> {
        unimplemented!()
    }

    async fn disconnect(&self) -> Result<()> {
        unimplemented!()
    }

    async fn ping(&self) -> Result<()> {
        unimplemented!()
    }

    async fn get_caps(&self) -> Result<Caps> {
        unimplemented!()
    }

    async fn get_sync(&self) -> Result<bool> {
        unimplemented!()
    }

    async fn get_active_connections(&self) -> Result<Connections> {
        unimplemented!()
    }

    fn trigger_abort(&self) -> Result<()> {
        unimplemented!()
    }
}
