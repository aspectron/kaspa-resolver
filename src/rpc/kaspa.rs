use super::Caps;
use crate::imports::*;
pub use kaspa_wrpc_client::{
    // client::{ConnectOptions, ConnectStrategy},
    KaspaRpcClient,
    //  WrpcEncoding,
};

pub struct Client {
    client: KaspaRpcClient,
}

#[async_trait]
impl rpc::Client for Client {
    fn try_new(encoding: WrpcEncoding, url: &str) -> Result<Self> {
        let client = KaspaRpcClient::new(encoding, Some(url), None, None, None)?;

        Ok(Self { client })
    }

    fn multiplexer(&self) -> Multiplexer<Ctl> {
        self.client.ctl_multiplexer()
    }

    async fn connect(&self, options: ConnectOptions) -> Result<()> {
        self.client.connect(Some(options)).await?;
        Ok(())
    }

    async fn get_caps(&self) -> Result<Caps> {
        let metrics = self
            .client
            .get_metrics(true, false, false, false, false, false)
            .await?;
        let process_metrics = metrics.process_metrics.ok_or(Error::Metrics)?;
        let socket_capacity = process_metrics
            .fd_num
            .min(process_metrics.core_num * rpc::SOCKETS_PER_CORE)
            as u64;
        Ok(Caps {
            resident_set_size: process_metrics.resident_set_size,
            core_num: process_metrics.core_num as u64,
            fd_num: process_metrics.fd_num as u64,
            socket_capacity,
        })
    }

    async fn get_sync(&self) -> Result<bool> {
        Ok(self.client.get_sync_status().await?)
    }

    async fn get_active_connections(&self) -> Result<u64> {
        let sockets = self.client.get_connections().await?;

        Ok(sockets as u64)
    }
}
