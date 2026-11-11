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

    #[error("Serialization failed: {0}")]
    SerializationFailed(#[source] DynError),

    #[error("Deserialization failed: {0}")]
    DeserializationFailed(#[source] DynError),

    #[error("URI parse error: {0}")]
    UriParseError(String),

    #[error("Invalid resource type: {0}")]
    InvalidResourceType(String),

    /// A keyset scan could not advance, so it would skip or refetch rows forever.
    #[error("{context}: keyset scan cannot advance ({detail})")]
    KeysetScanStalled { context: String, detail: String },

    #[error("Generic: {0}")]
    Generic(String),
}

/// Neo4j rejected a write that breaks a schema constraint. Retrying the batch can
/// never fix it, so it is a problem with that one event, not with the database.
const CONSTRAINT_VIOLATION_CODE: &str = "Neo.ClientError.Schema.ConstraintValidationFailed";

impl GraphError {
    pub fn should_not_retry_now(&self) -> bool {
        match self {
            GraphError::ConnectionNotInitialized => true,
            GraphError::QueryFailed(neo4rs::Error::Neo4j(e)) => !is_constraint_violation(e.code()),
            GraphError::QueryFailed(_) => true,
            _ => false,
        }
    }
}

fn is_constraint_violation(code: &str) -> bool {
    code == CONSTRAINT_VIOLATION_CODE
}

impl From<neo4rs::DeError> for GraphError {
    fn from(e: neo4rs::DeError) -> Self {
        GraphError::DeserializationFailed(Box::new(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_constraint_violation_code_matches() {
        assert!(is_constraint_violation(
            "Neo.ClientError.Schema.ConstraintValidationFailed"
        ));
        assert!(!is_constraint_violation(
            "Neo.TransientError.Transaction.DeadlockDetected"
        ));
        assert!(!is_constraint_violation(
            "Neo.ClientError.Statement.SyntaxError"
        ));
    }

    #[test]
    fn connection_level_query_failures_still_stop_the_batch() {
        assert!(GraphError::QueryFailed(neo4rs::Error::ConnectionError).should_not_retry_now());
        assert!(GraphError::ConnectionNotInitialized.should_not_retry_now());
        assert!(!GraphError::QueryTimeout.should_not_retry_now());
    }
}
