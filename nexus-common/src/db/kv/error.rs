use crate::types::DynError;
use thiserror::Error;

pub type RedisResult<T> = std::result::Result<T, RedisError>;

#[derive(Error, Debug)]
pub enum RedisError {
    #[error("Connection unavailable: {message}")]
    ConnectionUnavailable {
        message: String,
        recoverable: bool,
        #[source]
        source: Option<DynError>,
    },
    #[error("IO error: {0}")]
    IoError(#[source] DynError),
    #[error("Command failed: {0}")]
    CommandFailed(#[source] DynError),
    #[error("Serialization failed: {0}")]
    SerializationFailed(#[source] DynError),
    #[error("Deserialization failed: {0}")]
    DeserializationFailed(#[source] DynError),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

impl RedisError {
    pub fn is_recoverable(&self) -> bool {
        match self {
            RedisError::ConnectionUnavailable { recoverable, .. } => *recoverable,
            RedisError::IoError(_) => true,
            _ => false,
        }
    }

    pub fn from_serialization(e: serde_json::Error) -> Self {
        RedisError::SerializationFailed(Box::new(e))
    }

    pub fn from_deserialization(e: serde_json::Error) -> Self {
        RedisError::DeserializationFailed(Box::new(e))
    }
}

impl From<redis::RedisError> for RedisError {
    fn from(e: redis::RedisError) -> Self {
        if e.is_connection_refusal() || e.is_timeout() || e.is_io_error() {
            RedisError::IoError(Box::new(e))
        } else {
            RedisError::CommandFailed(Box::new(e))
        }
    }
}
