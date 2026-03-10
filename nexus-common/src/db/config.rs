use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use crate::StackConfig;

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
        DatabaseConfig::default().neo4j.clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct DatabaseConfig {
    pub redis: String,
    pub neo4j: Neo4JConfig,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        StackConfig::default().db.clone()
    }
}
