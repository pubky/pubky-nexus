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

    #[error("HsBlacklisted: {hs_id}")]
    HsBlacklisted { hs_id: String },

    #[error("Generic: {message}")]
    Generic {
        message: String,
        #[source]
        source: Option<Box<Self>>,
    },
}

impl From<neo4rs::DeError> for ModelError {
    fn from(e: neo4rs::DeError) -> Self {
        // Convert through GraphError to maintain error hierarchy
        ModelError::GraphOperationFailed(GraphError::from(e))
    }
}

impl ModelError {
    pub fn from_generic(source: impl std::fmt::Display) -> Self {
        Self::Generic {
            message: source.to_string(),
            source: None,
        }
    }

    /// Create a generic error that carries an underlying cause.
    pub fn from_generic_with_source(message: impl std::fmt::Display, source: Self) -> Self {
        Self::Generic {
            message: message.to_string(),
            source: Some(Box::new(source)),
        }
    }
}

pub type ModelResult<T> = Result<T, ModelError>;
