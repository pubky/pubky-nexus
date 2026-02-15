use neo4rs::DeError;
use thiserror::Error;

use crate::{db::kv::RedisError, media::processors::MediaProcessorError};

#[derive(Error, Debug)]
pub enum ModelError {
    /// Failed to perform Graph Operation
    #[error("GraphOperationFailed: {source}")]
    GraphOperationFailed {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    /// Failed to perform KV Operation
    #[error("KvOperationFailed: {source}")]
    KvOperationFailed {
        #[source]
        source: RedisError,
    },
    #[error("MediaProcessorError: {source}")]
    MediaProcessorError {
        #[source]
        source: MediaProcessorError,
    },
    #[error("FileOperationFailed: {source}")]
    FileOperationFailed {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Generic: {0}")]
    Generic(String),
}

impl From<RedisError> for ModelError {
    fn from(source: RedisError) -> Self {
        ModelError::KvOperationFailed { source }
    }
}

impl From<DeError> for ModelError {
    fn from(e: DeError) -> Self {
        ModelError::GraphOperationFailed {
            source: Box::new(e),
        }
    }
}

impl From<std::io::Error> for ModelError {
    fn from(e: std::io::Error) -> Self {
        ModelError::FileOperationFailed {
            source: Box::new(e),
        }
    }
}

impl From<neo4rs::Error> for ModelError {
    fn from(e: neo4rs::Error) -> Self {
        ModelError::GraphOperationFailed {
            source: Box::new(e),
        }
    }
}

impl From<MediaProcessorError> for ModelError {
    fn from(e: MediaProcessorError) -> Self {
        ModelError::MediaProcessorError { source: e }
    }
}

impl ModelError {
    pub fn from_graph_error(source: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::GraphOperationFailed {
            source: source.into(),
        }
    }

    pub fn from_generic(source: impl std::fmt::Display) -> Self {
        Self::Generic(source.to_string())
    }
}

pub type ModelResult<T> = Result<T, ModelError>;
