use serde::{Deserialize, Serialize};
use std::fmt::Debug;

mod neo4j;
pub use neo4j::Neo4JConfig;

pub const REDIS_URI: &str = "redis://localhost:6379";

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct DatabaseConfig {
    pub redis: String,
    pub neo4j: Neo4JConfig,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            redis: String::from(REDIS_URI),
            neo4j: Neo4JConfig::default(),
        }
    }
}
