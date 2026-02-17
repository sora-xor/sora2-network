use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("network profile '{0}' not found")]
    UnknownNetwork(String),
    #[error("unsupported network kind '{0}' for this tool")]
    UnsupportedNetworkKind(String),
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("rpc error: {0}")]
    Rpc(String),
    #[error("tool '{0}' is denied by policy")]
    ToolDenied(String),
}

pub type AppResult<T> = Result<T, AppError>;
