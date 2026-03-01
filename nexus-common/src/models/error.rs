use thiserror::Error;

use crate::{
    db::{kv::RedisError, GraphError},
    media::processors::MediaProcessorError,
};

#[derive(Error, Debug)]
pub enum ModelError {
    /// Failed to perform Graph Operation
    #[error("GraphOperationFailed")]
    GraphOperationFailed(#[from] GraphError),

    /// Failed to perform KV Operation
    #[error("KvOperationFailed")]
    KvOperationFailed(#[from] RedisError),

    #[error("MediaProcessorError")]
    MediaProcessorError(#[from] MediaProcessorError),

    #[error("FileOperationFailed")]
    FileOperationFailed(#[from] std::io::Error),

    #[error("Generic: {0}")]
    Generic(String),
}

impl From<neo4rs::DeError> for ModelError {
    fn from(e: neo4rs::DeError) -> Self {
        // Convert through GraphError to maintain error hierarchy
        ModelError::GraphOperationFailed(GraphError::from(e))
    }
}

impl ModelError {
    pub fn from_generic(source: impl std::fmt::Display) -> Self {
        Self::Generic(source.to_string())
    }
}

pub type ModelResult<T> = Result<T, ModelError>;
