use super::{redact_url, REDACTED};
use serde::{de::Error, Deserialize, Deserializer, Serialize};
use std::fmt;
use std::time::Duration;

pub const NEO4J_URI: &str = "bolt://localhost:7687";
pub const NEO4J_USER: &str = "neo4j";
pub const NEO4J_PASS: &str = "12345678";
const DEFAULT_NEO4J_REPLY_TIMEOUT_SECS: u64 = 15 * 60;
// Create temporal struct to wrap database config
#[derive(Clone, Serialize, Deserialize, Eq, PartialEq)]
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

    /// Longest wait in seconds for one reply from Neo4j, and for a TCP connect.
    /// A call whose reply takes longer, such as a GDS PageRank or a migration
    /// backfill, fails.
    #[serde(
        default = "default_neo4j_reply_timeout_secs",
        deserialize_with = "deserialize_nonzero_reply_timeout_secs"
    )]
    pub reply_timeout_secs: u64,
}

fn default_neo4j_user() -> String {
    String::from("neo4j")
}

const fn default_neo4j_reply_timeout_secs() -> u64 {
    DEFAULT_NEO4J_REPLY_TIMEOUT_SECS
}

fn deserialize_nonzero_reply_timeout_secs<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    match u64::deserialize(deserializer)? {
        0 => Err(D::Error::custom(
            "reply_timeout_secs must be at least 1 second",
        )),
        timeout => Ok(timeout),
    }
}

impl Neo4JConfig {
    /// Returns [`Self::reply_timeout_secs`] as a duration.
    pub fn reply_timeout(&self) -> Duration {
        Duration::from_secs(self.reply_timeout_secs)
    }
}

/// Redacts the password and the URI credentials, since the config is logged at startup
impl fmt::Debug for Neo4JConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            uri,
            user,
            password: _,
            slow_query_logging_threshold_ms,
            slow_query_logging_include_cypher,
            reply_timeout_secs,
        } = self;
        f.debug_struct("Neo4JConfig")
            .field("uri", &redact_url(uri))
            .field("user", user)
            .field("password", &REDACTED)
            .field(
                "slow_query_logging_threshold_ms",
                slow_query_logging_threshold_ms,
            )
            .field(
                "slow_query_logging_include_cypher",
                slow_query_logging_include_cypher,
            )
            .field("reply_timeout_secs", reply_timeout_secs)
            .finish()
    }
}

impl Default for Neo4JConfig {
    fn default() -> Self {
        Self {
            uri: String::from(NEO4J_URI),
            user: String::from(NEO4J_USER),
            password: String::from(NEO4J_PASS),
            slow_query_logging_threshold_ms: None,
            slow_query_logging_include_cypher: false,
            reply_timeout_secs: DEFAULT_NEO4J_REPLY_TIMEOUT_SECS,
        }
    }
}
