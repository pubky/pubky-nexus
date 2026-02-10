use serde::{Deserialize, Serialize};
use thiserror::Error;

//todokz: remove
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum DbError {
    /// Failed to execute query in the graph database
    #[error("GraphQueryFailed: {message}")]
    GraphQueryFailed { message: String },
    /// Failed to complete indexing due to a Redis write/del error
    #[error("IndexWriteFailed: Indexing incomplete due to Redis error - {message}")]
    IndexOperationFailed { message: String },
}
