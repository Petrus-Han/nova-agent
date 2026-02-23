use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("transport error: {0}")]
    Transport(String),

    #[error("invalid message: {0}")]
    InvalidMessage(String),

    #[error("connection closed")]
    ConnectionClosed,

    #[error("timeout after {0}ms")]
    Timeout(u64),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ProtocolError>;
