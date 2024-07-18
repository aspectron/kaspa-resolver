use super::Caps;
use crate::imports::*;
use kaspa_rpc_core::GetSystemInfoResponse;
pub use kaspa_wrpc_client::KaspaRpcClient;

#[derive(Debug)]
pub struct Client {
    client: KaspaRpcClient,
    url: String,
}

#[async_trait]
impl rpc::Client for Client {
    fn service() -> Service {
        Service::Kaspa
    }

    fn try_new(encoding: WrpcEncoding, url: &str) -> Result<Self> {
        let client = KaspaRpcClient::new(encoding, Some(url), None, None, None)?;

        Ok(Self {
            client,
            url: url.to_string(),
        })
    }

    fn multiplexer(&self) -> Multiplexer<Ctl> {
        self.client.ctl_multiplexer()
    }

    // async fn connect(&self, options: ConnectOptions) -> Result<()> {
    async fn connect(&self) -> Result<()> {
        let options = ConnectOptions {
            block_async_connect: false,
            strategy: ConnectStrategy::Retry,
            url: Some(self.url.clone()),
            ..Default::default()
        };

        self.client.connect(Some(options)).await?;
        Ok(())
    }

    async fn get_caps(&self) -> Result<Caps> {
        let GetSystemInfoResponse {
            system_id,
            cpu_physical_cores,
            total_memory,
            fd_limit,
        } = self.client.get_system_info().await?;
        let cpu_physical_cores = cpu_physical_cores as u64;
        let fd_limit = fd_limit as u64;
        let socket_capacity = fd_limit.min(cpu_physical_cores * rpc::SOCKETS_PER_CORE);
        // let system_id = u128::from_be_bytes(system_id[0..16].try_into()?);
        let system_id = u64::from_be_bytes(system_id[0..8].try_into()?);
        Ok(Caps {
            system_id,
            total_memory,
            cpu_physical_cores,
            fd_limit,
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
