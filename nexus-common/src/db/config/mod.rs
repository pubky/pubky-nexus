use serde::{Deserialize, Serialize};
use std::fmt::Debug;

mod neo4j;
pub use neo4j::Neo4JConfig;

pub const REDIS_URI: &str = "redis://localhost:6379";

/// Caps FT.SEARCH execution time well below RediSearch's 500ms default; ON_TIMEOUT RETURN yields partial results.
pub const FT_SEARCH_TIMEOUT_MS: usize = 50;

fn default_ft_search_timeout_ms() -> usize {
    FT_SEARCH_TIMEOUT_MS
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct DatabaseConfig {
    pub redis: String,
    pub neo4j: Neo4JConfig,

    /// Maximum time (ms) RediSearch spends on a single FT.SEARCH before returning partial results.
    #[serde(default = "default_ft_search_timeout_ms")]
    pub ft_search_timeout_ms: usize,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            redis: String::from(REDIS_URI),
            neo4j: Neo4JConfig::default(),
            ft_search_timeout_ms: default_ft_search_timeout_ms(),
        }
    }
}
