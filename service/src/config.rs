use dotenv::dotenv;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    neo4j_host: String,
    neo4j_port: String,
    pub neo4j_username: String,
    pub neo4j_password: String,
    _redis_host: String,
    _redis_port: String,
    pub static_path: String,
    server_host: String,
    server_port: String,
}

impl Config {
    pub fn from_env() -> Self {
        dotenv().ok();

        Self {
            neo4j_host: env::var("NEO4J_HOST").unwrap_or_else(|_| "localhost".to_string()),
            neo4j_port: env::var("NEO4J_PORT").unwrap_or_else(|_| "7687".to_string()),
            neo4j_username: env::var("NEO4J_DB_USERNAME").expect("NEO4J_DB_USERNAME not set"),
            neo4j_password: env::var("NEO4J_PASSWORD").expect("NEO4J_PASSWORD not set"),
            _redis_host: env::var("REDIS_HOST").unwrap_or_else(|_| "localhost".to_string()),
            _redis_port: env::var("REDIS_PORT").unwrap_or_else(|_| "6379".to_string()),
            static_path: env::var("STATIC_PATH").unwrap_or_else(|_| "./".to_string()),
            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            server_port: env::var("SERVER_PORT").unwrap_or_else(|_| "8080".to_string()),
        }
    }

    pub fn neo4j_uri(&self) -> String {
        format!("bolt://{}:{}", self.neo4j_host, self.neo4j_port)
    }

    pub fn _redis_uri(&self) -> String {
        format!("redis://{}:{}", self._redis_host, self._redis_port)
    }

    pub fn server_binding(&self) -> String {
        format!("{}:{}", self.server_host, self.server_port)
    }
}
