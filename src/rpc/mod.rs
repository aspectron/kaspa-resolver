pub mod kaspa;
pub mod sparkle;

use crate::imports::*;

const SOCKETS_PER_CORE: u64 = 1024;

#[allow(dead_code)]
#[derive(Debug)]
pub struct Caps {
    // node system id
    pub system_id: u64,
    // git hash
    pub git_hash: Option<String>,
    // current memory usage in bytes
    pub total_memory: u64,
    // number of cores
    pub cpu_physical_cores: u64,
    // number of utilized file descriptors
    pub fd_limit: u64,
    // number of available sockets
    pub socket_capacity: u64,
}

impl Caps {
    pub fn system_id(&self) -> u64 {
        self.system_id
    }
}

#[async_trait]
pub trait Client: std::fmt::Debug + Sized + Send + Sync + 'static {
    fn try_new(_encoding: WrpcEncoding, _url: &str) -> Result<Self> {
        todo!()
    }

    fn service() -> Service;

    fn multiplexer(&self) -> Multiplexer<Ctl> {
        todo!()
    }

    async fn connect(&self) -> Result<()> {
        todo!()
    }

    async fn get_caps(&self) -> Result<Caps> {
        todo!()
    }

    async fn get_sync(&self) -> Result<bool> {
        todo!()
    }

    async fn get_active_connections(&self) -> Result<u64> {
        todo!()
    }
}
