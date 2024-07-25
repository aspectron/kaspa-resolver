use super::Caps;
use crate::imports::*;
use kaspa_rpc_core::GetSystemInfoResponse;
pub use kaspa_wrpc_client::KaspaRpcClient;

// reduce fd_limit by this amount to ensure the
// system has enough file descriptors for other
// tasks (peers, db, etc)
// while default kHOST setup is:
// outgoing peers: 256
// incoming peers: 32
// peers are included the reported
// node connection count
// reserved for db etc.: 1024
const FD_MARGIN: u64 = 1024;

#[derive(Debug)]
pub struct Client {
    client: KaspaRpcClient,
    url: String,
}

impl Client {
    pub fn try_new(encoding: WrpcEncoding, url: &str) -> Result<Self> {
        let client = KaspaRpcClient::new(encoding, Some(url), None, None, None)?;

        Ok(Self {
            client,
            url: url.to_string(),
        })
    }
}

impl rpc::ClientT for Client {
    fn multiplexer(&self) -> Multiplexer<Ctl> {
        self.client.ctl_multiplexer()
    }

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

    async fn disconnect(&self) -> Result<()> {
        Ok(self.client.disconnect().await?)
    }

    async fn ping(&self) -> Result<()> {
        Ok(self.client.ping().await?)
    }

    async fn get_caps(&self) -> Result<Caps> {
        let GetSystemInfoResponse {
            version,
            system_id,
            git_hash,
            cpu_physical_cores,
            total_memory,
            fd_limit,
        } = self.client.get_system_info().await?;
        let cpu_physical_cores = cpu_physical_cores as u64;
        let fd_limit = fd_limit as u64;
        // reduce node's fd_limit by FD_MARGIN to ensure
        // the system has enough file descriptors for other
        // tasks (peers, db, etc)
        let fd_limit_actual = fd_limit.checked_sub(FD_MARGIN).unwrap_or(32);
        // by default we assume that the node is able to accept
        // 1024 connections per core (default NGINX worker configuration)
        // TODO: this should be increased in the future once a custom
        // proxy is implemented
        let socket_capacity = fd_limit_actual.min(cpu_physical_cores * rpc::SOCKETS_PER_CORE);
        let system_id = system_id
            .and_then(|v| v[0..8].try_into().ok().map(u64::from_be_bytes))
            .unwrap_or_default();
        // let system_id_hex_string = format!("{:016x}", system_id);
        let git_hash = git_hash.as_ref().map(ToHex::to_hex);
        Ok(Caps {
            version,
            system_id,
            git_hash,
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

    fn trigger_abort(&self) -> Result<()> {
        Ok(self.client.trigger_abort()?)
    }
}
