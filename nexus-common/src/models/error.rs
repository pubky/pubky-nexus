use neo4rs::DeError;
use thiserror::Error;

use crate::db::kv::RedisError;

#[derive(Error, Debug)]
pub enum ModelError {
    /// Failed to perform Graph Operation
    #[error("GraphOperationFailed: {message}")]
    GraphOperationFailed { message: String },
    /// Failed to perform KV Operation
    #[error("KvOperationFailed: {message}")]
    KvOperationFailed { message: String },
    #[error("FileOperationFailed: {message}")]
    FileOperationFailed { message: String },
    #[error("Other: {message}")]
    Other { message: String },
}

impl From<RedisError> for ModelError {
    fn from(e: RedisError) -> Self {
        ModelError::KvOperationFailed {
            message: e.to_string(),
        }
    }
}

impl From<DeError> for ModelError {
    fn from(e: DeError) -> Self {
        ModelError::GraphOperationFailed {
            message: e.to_string(),
        }
    }
}

impl ModelError {
    pub fn from_graph_error(source: impl std::fmt::Display) -> Self {
        Self::GraphOperationFailed {
            message: source.to_string(),
        }
    }

    pub fn from_file_operation(source: impl std::fmt::Display) -> Self {
        Self::FileOperationFailed {
            message: source.to_string(),
        }
    }

    pub fn from_other(source: impl std::fmt::Display) -> Self {
        Self::Other {
            message: source.to_string(),
        }
    }
}
