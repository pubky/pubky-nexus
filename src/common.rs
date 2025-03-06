// TOOD: #revisit, moved to common/src/db/config.rs

// use std::{
//     fmt::Debug,
//     path::{Path, PathBuf},
// };

// use async_trait::async_trait;
// use serde::{de::DeserializeOwned, Deserialize, Serialize};
// use tokio::fs;

// use crate::types::DynError;

// pub const REDIS_URI: &str = "redis://localhost:6379";
// pub const NEO4J_URI: &str = "bolt://localhost:7687";
// pub const NEO4J_USER: &str = "neo4j";
// pub const NEO4J_PASS: &str = "12345678";

// pub const LOG_LEVEL: Level = Level::Debug;
// pub const FILES_DIR: &str = "./static/files";

// #[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
// #[serde(rename_all = "lowercase")]
// pub enum Level {
//     /// Designates very low priority, often extremely verbose, information.
//     Trace,
//     /// Designates lower priority information.
//     Debug,
//     /// Designates useful information.
//     Info,
//     /// Designates hazardous situations.
//     Warn,
//     /// Designates very serious errors.
//     Error,
// }

// impl Level {
//     pub fn as_str(&self) -> &'static str {
//         match self {
//             Level::Trace => "trace",
//             Level::Debug => "debug",
//             Level::Info => "info",
//             Level::Warn => "warn",
//             Level::Error => "error",
//         }
//     }
// }

// #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
// pub struct Config {
//     pub name: String,
//     pub log_level: Level,
//     pub otlp_endpoint: Option<String>,
//     pub db: DatabaseConfig,
//     pub files_path: PathBuf,
// }

// impl Config {
//     pub fn default(name: String) -> Self {
//         Self {
//             name,
//             log_level: LOG_LEVEL,
//             files_path: PathBuf::from(FILES_DIR),
//             otlp_endpoint: None,
//             db: DatabaseConfig::default(),
//         }
//     }
// }

// // Create temporal struct to wrap database config
// #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
// pub struct Neo4JConfig {
//     pub uri: String,
//     #[serde(default = "default_neo4j_user")]
//     pub user: String,
//     pub password: String,
// }

// fn default_neo4j_user() -> String {
//     String::from("neo4j")
// }

// impl Default for Neo4JConfig {
//     fn default() -> Self {
//         Self {
//             uri: String::from(NEO4J_URI),
//             user: String::from(NEO4J_USER),
//             password: String::from(NEO4J_PASS),
//         }
//     }
// }

// #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
// pub struct DatabaseConfig {
//     pub redis: String,
//     pub neo4j: Neo4JConfig,
// }

// impl Default for DatabaseConfig {
//     fn default() -> Self {
//         Self {
//             redis: String::from(REDIS_URI),
//             neo4j: Neo4JConfig::default(),
//         }
//     }
// }

// #[async_trait]
// pub trait ConfigLoader<T>
// where
//     T: DeserializeOwned + Send + Sync + Debug,
// {
//     /// Parses the struct from a TOML string
//     fn try_from_str(value: &str) -> Result<T, DynError> {
//         let config_toml: T = toml::from_str(value)?;
//         Ok(config_toml)
//     }

//     /// Loads the struct from a TOML file
//     async fn load(path: impl AsRef<Path> + Send) -> Result<T, DynError> {
//         let config_file_path = path.as_ref();

//         // Read file with error handling
//         let s = fs::read_to_string(config_file_path)
//             .await
//             .map_err(|e| format!("Failed to read config file {:?}: {}", config_file_path, e))?;

//         // Convert TOML to struct with error handling
//         let config = Self::try_from_str(&s)
//             .map_err(|e| format!("Failed to parse config file {:?}: {}", config_file_path, e))?;

//         Ok(config)
//     }
// }
