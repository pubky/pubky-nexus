use serde::{Deserialize, Serialize};

pub const NEO4J_URI: &str = "bolt://localhost:7687";
pub const NEO4J_USER: &str = "neo4j";
pub const NEO4J_PASS: &str = "12345678";
pub const DEFAULT_SLOW_QUERY_THRESHOLD_MS: u64 = 100;

// Create temporal struct to wrap database config
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

fn default_neo4j_user() -> String {
    String::from("neo4j")
}

fn default_slow_query_logging_threshold_ms() -> u64 {
    DEFAULT_SLOW_QUERY_THRESHOLD_MS
}

fn default_slow_query_logging_enabled() -> bool {
    true
}

impl Default for Neo4JConfig {
    fn default() -> Self {
        Self {
            uri: String::from(NEO4J_URI),
            user: String::from(NEO4J_USER),
            password: String::from(NEO4J_PASS),
            slow_query_logging_threshold_ms: DEFAULT_SLOW_QUERY_THRESHOLD_MS,
            slow_query_logging_enabled: true,
            slow_query_logging_include_cypher: false,
        }
    }
}
