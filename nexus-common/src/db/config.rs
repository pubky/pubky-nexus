use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::sync::OnceLock;

use crate::config::file::reader::DEFAULT_CONFIG_TOML;

/// Intermediate structure to extract database config defaults from the TOML
#[derive(Deserialize)]
struct DefaultConfigParsed {
    stack: StackParsed,
}

#[derive(Deserialize)]
struct StackParsed {
    db: DbParsed,
}

#[derive(Deserialize)]
struct DbParsed {
    redis: String,
    neo4j: Neo4JConfig,
}

static DEFAULT_DB_CONFIG: OnceLock<DbParsed> = OnceLock::new();

/// Returns the default database config parsed from `default.config.toml`
fn get_default_db_config() -> &'static DbParsed {
    DEFAULT_DB_CONFIG.get_or_init(|| {
        let config: DefaultConfigParsed = toml::from_str(DEFAULT_CONFIG_TOML).expect(
            "embedded default.config.toml should be valid TOML and contain [stack.db] section",
        );
        config.stack.db
    })
}

/// Returns the default Redis URI from `default.config.toml`.
fn default_redis_uri() -> String {
    get_default_db_config().redis.clone()
}

/// Returns the default Neo4j config from `default.config.toml`
fn default_neo4j_config() -> Neo4JConfig {
    get_default_db_config().neo4j.clone()
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
