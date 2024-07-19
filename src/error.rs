use thiserror::Error;
use toml::de::Error as TomlError;

#[derive(Error, Debug)]
pub enum Error {
    // #[error("{0}")]
    // Custom(String),
    #[error("RPC error: {0}")]
    Rpc(#[from] kaspa_wrpc_client::error::Error),

    #[error(transparent)]
    SparkleRpc(#[from] sparkle_rpc_client::error::Error),

    #[error("TOML error: {0}")]
    Toml(#[from] TomlError),

    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    #[error("Metrics")]
    Metrics,

    #[error("Sync")]
    Sync,

    #[error("Status")]
    Status,

    #[error("Channel send error")]
    ChannelSend,

    #[error("Channel try send error")]
    TryChannelSend,

    #[error(transparent)]
    Encryption(#[from] workflow_encryption::error::Error),

    #[error(transparent)]
    KaspaRpc(#[from] kaspa_rpc_core::RpcError),

    #[error("Incompatible connection protocol encoding")]
    ConnectionProtocolEncoding,

    #[error("Configuration error")]
    Config(String),

    #[error(transparent)]
    TryFromSlice(#[from] std::array::TryFromSliceError),

    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error("Could not locate local config")]
    LocalConfigNotFound,
}

// impl Error {
//     pub fn custom<T: std::fmt::Display>(msg: T) -> Self {
//         Error::Custom(msg.to_string())
//     }
// }

impl Error {
    pub fn config<T: std::fmt::Display>(msg: T) -> Self {
        Error::Config(msg.to_string())
    }
}

impl<T> From<workflow_core::channel::SendError<T>> for Error {
    fn from(_: workflow_core::channel::SendError<T>) -> Self {
        Error::ChannelSend
    }
}

impl<T> From<workflow_core::channel::TrySendError<T>> for Error {
    fn from(_: workflow_core::channel::TrySendError<T>) -> Self {
        Error::TryChannelSend
    }
}
