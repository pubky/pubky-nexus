use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::OnceLock;

use crate::config::file::reader::DEFAULT_CONFIG_TOML;
use crate::config::file::validate_and_expand_path;
use crate::config::Level;

/// Intermediate structure to extract stack config defaults from the TOML
#[derive(Deserialize)]
struct DefaultConfigParsed {
    stack: StackConfigParsed,
}

/// Parsed stack config from TOML (before path expansion)
#[derive(Deserialize, Clone)]
pub(crate) struct StackConfigParsed {
    log_level: Level,
    files_path: PathBuf,
    otlp_endpoint: Option<String>,
    db: DatabaseConfig,
}

static DEFAULT_STACK_CONFIG: OnceLock<StackConfigParsed> = OnceLock::new();

/// Returns the default stack config parsed from `default.config.toml`
pub(crate) fn get_default_stack_config() -> &'static StackConfigParsed {
    DEFAULT_STACK_CONFIG.get_or_init(|| {
        let config: DefaultConfigParsed = toml::from_str(DEFAULT_CONFIG_TOML).expect(
            "embedded default.config.toml should be valid TOML and contain [stack] section",
        );
        config.stack
    })
}

/// Returns the default log level from `default.config.toml`
pub(crate) fn default_log_level() -> Level {
    get_default_stack_config().log_level
}

/// Returns the default files path from `default.config.toml` (with ~ expansion)
pub(crate) fn default_files_path() -> PathBuf {
    validate_and_expand_path(get_default_stack_config().files_path.clone())
        .expect("default.config.toml files_path should be a valid directory path")
}

/// Returns the default OTLP endpoint from `default.config.toml`
pub(crate) fn default_otlp_endpoint() -> Option<String> {
    get_default_stack_config().otlp_endpoint.clone()
}

/// Returns the default Redis URI from `default.config.toml`.
fn default_redis_uri() -> String {
    get_default_stack_config().db.redis.clone()
}

/// Returns the default Neo4j config from `default.config.toml`
fn default_neo4j_config() -> Neo4JConfig {
    get_default_stack_config().db.neo4j.clone()
}

fn default_neo4j_user() -> String {
    String::from("neo4j")
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Neo4JConfig {
    pub uri: String,
    #[serde(default = "default_neo4j_user")]
    pub user: String,
    pub password: String,
}

impl Default for Neo4JConfig {
    fn default() -> Self {
        default_neo4j_config()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct DatabaseConfig {
    pub redis: String,
    pub neo4j: Neo4JConfig,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            redis: default_redis_uri(),
            neo4j: Neo4JConfig::default(),
        }
    }
}
