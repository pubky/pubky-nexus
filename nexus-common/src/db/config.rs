use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use crate::StackConfig;

// TODO Does this need to be pub? Isn't this only relevant for default deser where JSOn field isn't set?
pub const DEFAULT_SLOW_QUERY_THRESHOLD_MS: u64 = 100;

fn default_neo4j_user() -> String {
    String::from("neo4j")
}

fn default_slow_query_logging_threshold_ms() -> u64 {
    DEFAULT_SLOW_QUERY_THRESHOLD_MS
}

fn default_slow_query_logging_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Neo4JConfig {
    pub uri: String,

    #[serde(default = "default_neo4j_user")]
    pub user: String,

    pub password: String,

    /// Queries exceeding this threshold (in milliseconds) are logged as warnings.
    /// Only used when `slow_query_logging_enabled` is true.
    #[serde(default = "default_slow_query_logging_threshold_ms")]
    pub slow_query_logging_threshold_ms: u64,

    /// Enable slow-query logging. Defaults to true.
    /// Set to false for CLI/admin commands where tracing overhead is unnecessary.
    #[serde(default = "default_slow_query_logging_enabled")]
    pub slow_query_logging_enabled: bool,

    /// Include the full cypher (with interpolated params) in slow-query warnings.
    /// Useful for debugging but verbose. Defaults to false.
    #[serde(default)]
    pub slow_query_logging_include_cypher: bool,
}

impl Default for Neo4JConfig {
    /// Extracted from `default.config.toml` via [StackConfig] > [DatabaseConfig] > [Neo4JConfig]
    fn default() -> Self {
        DatabaseConfig::default().neo4j
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct DatabaseConfig {
    pub redis: String,
    pub neo4j: Neo4JConfig,
}

impl Default for DatabaseConfig {
    /// Extracted from `default.config.toml` via [StackConfig] > [DatabaseConfig]
    fn default() -> Self {
        StackConfig::default().db
    }
}
