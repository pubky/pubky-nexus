use neo4rs::DeError;
use thiserror::Error;

use crate::db::kv::RedisError;

#[derive(Error, Debug)]
pub enum ModelError {
    /// Failed to perform Graph Operation
    #[error("GraphOperationFailed: {message}")]
    GraphOperationFailed {
        message: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    /// Failed to perform KV Operation
    #[error("KvOperationFailed: {message}")]
    KvOperationFailed {
        message: String,
        #[source]
        source: RedisError,
    },
    #[error("FileOperationFailed: {message}")]
    FileOperationFailed {
        message: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Other: {message}")]
    Other { message: String },
}

impl From<RedisError> for ModelError {
    fn from(e: RedisError) -> Self {
        let message = e.to_string();
        ModelError::KvOperationFailed { message, source: e }
    }
}

impl From<DeError> for ModelError {
    fn from(e: DeError) -> Self {
        let message = e.to_string();
        ModelError::GraphOperationFailed {
            message,
            source: Box::new(e),
        }
    }
}

impl From<std::io::Error> for ModelError {
    fn from(e: std::io::Error) -> Self {
        let message = e.to_string();
        ModelError::FileOperationFailed {
            message,
            source: Box::new(e),
        }
    }
}

impl From<neo4rs::Error> for ModelError {
    fn from(e: neo4rs::Error) -> Self {
        let message = e.to_string();
        ModelError::GraphOperationFailed {
            message,
            source: Box::new(e),
        }
    }
}

impl ModelError {
    pub fn from_graph_error(source: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        let source = source.into();
        let message = source.to_string();
        Self::GraphOperationFailed { message, source }
    }

    pub fn from_file_operation(
        source: impl Into<Box<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        let source = source.into();
        let message = source.to_string();
        Self::FileOperationFailed { message, source }
    }

    pub fn from_other(source: impl std::fmt::Display) -> Self {
        Self::Other {
            message: source.to_string(),
        }
    }
}
