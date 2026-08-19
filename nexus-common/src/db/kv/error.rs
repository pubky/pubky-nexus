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

impl RedisError {
    pub fn should_not_retry_now(&self) -> bool {
        matches!(
            self,
            RedisError::ConnectionNotInitialized
                | RedisError::ConnectionPoolError(_)
                | RedisError::IoError(_)
        )
    }
}

/// Shared monotonicity rule for Primary HS global cursors and External HS user cursors:
/// a cursor may hold or advance, but never rewind. Equal is allowed so an
/// idempotent re-persist of a boundary event is not treated as an error.
pub fn ensure_cursor_not_backwards(new_cursor: u64, stored_cursor: u64) -> RedisResult<()> {
    if new_cursor < stored_cursor {
        return Err(RedisError::InvalidInput(
            "Cursor cannot move backwards".into(),
        ));
    }
    Ok(())
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
