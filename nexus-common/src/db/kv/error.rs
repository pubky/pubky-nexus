use crate::types::DynError;
use thiserror::Error;

pub type RedisResult<T> = std::result::Result<T, RedisError>;

#[derive(Error, Debug)]
pub enum RedisError {
    #[error("Connection not initialized")]
    ConnectionNotInitialized,
    #[error("Connection pool error")]
    ConnectionPoolError(#[source] DynError),
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

impl From<redis::RedisError> for RedisError {
    fn from(e: redis::RedisError) -> Self {
        if e.is_connection_refusal() || e.is_timeout() || e.is_io_error() {
            RedisError::IoError(Box::new(e))
        } else {
            RedisError::CommandFailed(Box::new(e))
        }
    }
}
