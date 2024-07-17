use crate::imports::*;

#[derive(Clone, Debug)]
pub enum Delegate {
    Wrpc(Arc<Connection<rpc::kaspa::Client>>),
    // WrpcJson(Arc<Connection<WrpcJson>>),
    // Grpc(Arc<Connection<Grpc>>),
}

impl From<Connection<rpc::kaspa::Client>> for Delegate {
    fn from(connection: Connection<rpc::kaspa::Client>) -> Self {
        Self::Wrpc(Arc::new(connection))
    }
}
