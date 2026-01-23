use crate::types::DynError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RedisError {
    #[error("Connection unavailable: {0}")]
    ConnectionUnavailable(#[source] DynError),
    #[error("Command failed: {0}")]
    CommandFailed(#[source] DynError),
    #[error("Serialization failed: {0}")]
    SerializationFailed(#[source] DynError),
    #[error("Deserialization failed: {0}")]
    DeserializationFailed(#[source] DynError),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}
