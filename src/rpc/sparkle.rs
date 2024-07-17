use crate::imports::*;
use sparkle_rpc_client::prelude::SparkleRpcClient;

#[derive(Debug)]
pub struct Client {
    client: SparkleRpcClient,
}

#[async_trait]
impl rpc::Client for Client {
    fn service() -> Service {
        Service::Sparkle
    }

    fn try_new(encoding: WrpcEncoding, url: &str) -> Result<Self> {
        let client = SparkleRpcClient::try_new(url, Some(encoding))?;

        Ok(Self { client })
    }

    fn multiplexer(&self) -> Multiplexer<Ctl> {
        self.client.ctl_multiplexer()
    }

    async fn connect(&self, options: ConnectOptions) -> Result<()> {
        self.client.connect(Some(options)).await?;
        Ok(())
    }

    // async fn get_caps(&self) -> Result<Caps> {
    //     let metrics = self.client.get_metrics(true, false, false, false, false).await?;
    //     let process_metrics = metrics.process_metrics.ok_or(Error::Metrics)?;

    //     Ok(Caps {
    //         resident_set_size: process_metrics.resident_set_size,
    //         core_num: process_metrics.core_num,
    //         fd_num: process_metrics.fd_num,
    //     })
    // }

    // async fn get_sync(&self) -> Result<bool> {
    //     Ok(self.client.get_sync_status().await?)
    // }

    // async fn get_active_connections(&self) -> Result<u64> {

    //     let metrics = self.client.get_metrics(false, true, false, false, false).await?;
    //     let connection_metrics = metrics.connection_metrics.ok_or(Error::Metrics)?;
    //     let sockets =
    //         connection_metrics.borsh_live_connections
    //         + connection_metrics.json_live_connections
    //         + connection_metrics.active_peers
    //         ;

    //     Ok(sockets as u64)
    // }
}
