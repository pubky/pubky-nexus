use thiserror::Error;

use crate::{
    db::{kv::RedisError, GraphError},
    media::processors::MediaProcessorError,
};

#[derive(Error, Debug)]
pub enum ModelError {
    /// Failed to perform Graph Operation
    #[error("GraphOperationFailed")]
    GraphOperationFailed {
        #[source]
        source: GraphError,
    },
    /// Failed to perform KV Operation
    #[error("KvOperationFailed")]
    KvOperationFailed {
        #[source]
        source: RedisError,
    },
    #[error("MediaProcessorError")]
    MediaProcessorError {
        #[source]
        source: MediaProcessorError,
    },
    #[error("FileOperationFailed")]
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

impl From<std::io::Error> for ModelError {
    fn from(e: std::io::Error) -> Self {
        ModelError::FileOperationFailed {
            source: Box::new(e),
        }
    }
}

impl From<MediaProcessorError> for ModelError {
    fn from(e: MediaProcessorError) -> Self {
        ModelError::MediaProcessorError { source: e }
    }
}

impl From<GraphError> for ModelError {
    fn from(e: GraphError) -> Self {
        ModelError::GraphOperationFailed { source: e }
    }
}

impl From<neo4rs::DeError> for ModelError {
    fn from(e: neo4rs::DeError) -> Self {
        // Convert through GraphError to maintain error hierarchy
        ModelError::GraphOperationFailed {
            source: GraphError::from(e),
        }
    }
}

impl ModelError {
    pub fn from_generic(source: impl std::fmt::Display) -> Self {
        Self::Generic(source.to_string())
    }
}

pub type ModelResult<T> = Result<T, ModelError>;
