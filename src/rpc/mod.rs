pub mod kaspa;
pub mod sparkle;

use crate::imports::*;

#[allow(dead_code)]
const TARGET_RAM: u64 = 1024 * 1024 * 1024 * 16;

const SOCKETS_PER_CORE: u32 = 1024;

#[allow(dead_code)]
#[derive(Debug)]
pub struct Caps {
    pub resident_set_size: u64,
    pub core_num: u64,
    pub fd_num: u64,
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
