pub mod kaspa;
pub mod sparkle;

use crate::imports::*;

const SOCKETS_PER_CORE: u64 = 1024;

#[allow(dead_code)]
#[derive(Debug)]
pub struct Caps {
    // node id
    pub system_id: Vec<u8>,
    // node id in hex
    pub hex_id: String,
    // current memory usage in bytes
    pub total_memory: u64,
    // number of cores
    pub cpu_physical_cores: u64,
    // number of utilized file descriptors
    pub fd_limit: u64,
    // number of available sockets
    pub socket_capacity: u64,
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

    async fn connect(&self, _options: ConnectOptions) -> Result<()> {
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
