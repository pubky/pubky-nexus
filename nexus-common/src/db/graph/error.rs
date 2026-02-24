use thiserror::Error;

use crate::types::DynError;

pub type GraphResult<T> = std::result::Result<T, GraphError>;

#[derive(Error, Debug)]
pub enum GraphError {
    #[error("Connection not initialized")]
    ConnectionNotInitialized,
    #[error("Query failed: {0}")]
    QueryFailed(#[from] neo4rs::Error),
    #[error("Query timeout")]
    QueryTimeout,
    #[error("Query build error: {0}")]
    QueryBuildError(String),
    #[error("Serialization failed")]
    SerializationFailed(#[source] DynError),
    #[error("Deserialization failed: {0}")]
    DeserializationFailed(#[from] neo4rs::DeError),
    #[error("URI parse error: {0}")]
    UriParseError(String),
    #[error("Invalid resource type: {0}")]
    InvalidResourceType(String),
    #[error("Generic: {0}")]
    Generic(String),
}
