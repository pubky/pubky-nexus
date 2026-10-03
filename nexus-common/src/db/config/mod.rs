use serde::{Deserialize, Serialize};
use std::fmt;

mod neo4j;
mod redact;
pub use neo4j::Neo4JConfig;
pub use redact::{redact_url, REDACTED};

pub const REDIS_URI: &str = "redis://localhost:6379";

/// Caps FT.SEARCH execution time well below RediSearch's 500ms default; ON_TIMEOUT RETURN yields partial results.
pub const FT_SEARCH_TIMEOUT_MS: usize = 50;

fn default_ft_search_timeout_ms() -> usize {
    FT_SEARCH_TIMEOUT_MS
}

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct DatabaseConfig {
    pub redis: String,
    pub neo4j: Neo4JConfig,

    /// Maximum time (ms) RediSearch spends on a single FT.SEARCH before returning partial results.
    #[serde(default = "default_ft_search_timeout_ms")]
    pub ft_search_timeout_ms: usize,
}

/// Redacts the Redis URL, since the config is logged at startup
impl fmt::Debug for DatabaseConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            redis,
            neo4j,
            ft_search_timeout_ms,
        } = self;
        f.debug_struct("DatabaseConfig")
            .field("redis", &redact_url(redis))
            .field("neo4j", neo4j)
            .field("ft_search_timeout_ms", ft_search_timeout_ms)
            .finish()
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_does_not_leak_the_credentials() {
        let config = DatabaseConfig {
            redis: "redis://default:redis-s3cret@cache.example.com:6379".into(),
            neo4j: Neo4JConfig {
                uri: "neo4j+s://neo4j:uri-s3cret@graph.example.com:7687".into(),
                password: "neo4j-s3cret".into(),
                ..Default::default()
            },
            ..Default::default()
        };

        let debug = format!("{config:?}");

        assert!(!debug.contains("s3cret"), "credentials leaked: {debug}");
        assert!(debug.contains("redis://***@cache.example.com:6379"));
        assert!(debug.contains("neo4j+s://***@graph.example.com:7687"));
    }
}
