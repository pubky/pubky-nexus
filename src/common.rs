use std::path::PathBuf;

use tracing::Level;

pub const REDIS_URI: &str = "redis://localhost:6379";
pub const NEO4J_URI: &str = "bolt://localhost:7687";
pub const NEO4J_USER: &str = "neo4j";
pub const NEO4J_PASS: &str = "12345678";

pub const LOG_LEVEL: Level = Level::DEBUG;
pub const FILES_DIR: &str = "./static/files";

#[derive(Debug, Clone)]
pub struct Config {
    pub name: String,
    pub log_level: Level,
    pub otlp_endpoint: Option<String>,
    pub db: DatabaseConfig,
    pub files_path: PathBuf,
}

impl Config {
    pub fn default(name: String) -> Self {
        Self {
            name,
            log_level: LOG_LEVEL,
            files_path: PathBuf::from(FILES_DIR),
            otlp_endpoint: None,
            db: DatabaseConfig::default(),
        }
    }
}

// Create temporal struct to wrap database config
#[derive(Debug, Clone)]
pub struct Neo4JConfig {
    pub uri: String,
    pub user: String,
    pub password: String,
}

impl Default for Neo4JConfig {
    fn default() -> Self {
        Self {
            uri: String::from(NEO4J_URI),
            user: String::from(NEO4J_USER),
            password: String::from(NEO4J_PASS),
        }
    }
}

#[derive(Debug, Clone)]
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