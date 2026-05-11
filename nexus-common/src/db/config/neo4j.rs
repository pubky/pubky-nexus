use serde::{Deserialize, Serialize};

pub const NEO4J_URI: &str = "bolt://localhost:7687";
pub const NEO4J_USER: &str = "neo4j";
pub const NEO4J_PASS: &str = "12345678";
// Create temporal struct to wrap database config
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Neo4JConfig {
    pub uri: String,

    #[serde(default = "default_neo4j_user")]
    pub user: String,

    pub password: String,

    /// Slow-query warning threshold in milliseconds.
    /// `Some(ms)` — emit a warning for queries exceeding `ms` milliseconds.
    /// `Some(0)`  — warn on every query (useful for debugging).
    /// `None`     — disable slow-query warnings.
    /// Defaults to `None` (disabled).
    #[serde(default)]
    pub slow_query_logging_threshold_ms: Option<u64>,

    /// Include the full cypher (with interpolated params) in slow-query warnings.
    /// Useful for debugging but verbose. Defaults to false.
    #[serde(default)]
    pub slow_query_logging_include_cypher: bool,
}

fn default_neo4j_user() -> String {
    String::from("neo4j")
}

impl Default for Neo4JConfig {
    fn default() -> Self {
        Self {
            uri: String::from(NEO4J_URI),
            user: String::from(NEO4J_USER),
            password: String::from(NEO4J_PASS),
            slow_query_logging_threshold_ms: None,
            slow_query_logging_include_cypher: false,
        }
    }
}
