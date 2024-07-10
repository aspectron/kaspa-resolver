pub mod kaspa;
pub mod sparkle;

use crate::imports::*;

const SOCKETS_PER_CORE: u32 = 1024;

#[allow(dead_code)]
#[derive(Debug)]
pub struct Caps {
    // current memory usage in bytes
    pub resident_set_size: u64,
    // number of cores
    pub core_num: u64,
    // number of utilized file descriptors
    pub fd_num: u64,
    // number of available sockets
    pub socket_capacity: u64,
}

#[async_trait]
pub trait Client: Sized {
    fn try_new(_encoding: WrpcEncoding, _url: &str) -> Result<Self> {
        todo!()
    }

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
