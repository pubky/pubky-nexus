use dotenv::dotenv;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    neo4j_host: String,
    neo4j_port: String,
    pub neo4j_username: String,
    pub neo4j_password: String,
    redis_host: String,
    redis_port: String,
    pub static_path: String,
    pub file_path: String,
    pub base_file_url: String,
    pub server_host: String,
    pub server_port: String,
    pub reindex: bool,
    pub testnet: bool,
    pub homeserver: String,
    pub events_limit: u32,
    pub watcher_sleep: u64,
    pub max_retries: u64,
}

impl Config {
    pub fn from_env() -> Self {
        dotenv().ok();

        Self {
            neo4j_host: env::var("NEO4J_HOST").unwrap_or_else(|_| "localhost".to_string()),
            neo4j_port: env::var("NEO4J_PORT").unwrap_or_else(|_| "7687".to_string()),
            redis_host: env::var("REDIS_HOST").unwrap_or_else(|_| "localhost".to_string()),
            redis_port: env::var("REDIS_PORT").unwrap_or_else(|_| "6379".to_string()),
            static_path: env::var("STATIC_PATH").unwrap_or_else(|_| "./static".to_string()),
            file_path: env::var("FILE_PATH").unwrap_or_else(|_| "./static/files".to_string()),
            base_file_url: env::var("BASE_FILE_URL")
                .unwrap_or_else(|_| "127.0.0.1:8080/static/files/".to_string()),
            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            server_port: env::var("SERVER_PORT").unwrap_or_else(|_| "8080".to_string()),
            reindex: env::var("REINDEX")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            testnet: env::var("TESTNET")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(true),
            homeserver: env::var("HOMESERVER").expect("HOMESERVER pubky id not set"),
            events_limit: env::var("EVENTS_LIMIT")
                .unwrap_or("1000".to_string())
                .parse()
                .unwrap_or(1000),
            watcher_sleep: env::var("WATCHER_SLEEP")
                .unwrap_or("5000".to_string())
                .parse()
                .unwrap_or(5000),
            max_retries: env::var("MAX_RETRIES")
                .unwrap_or("1".to_string())
                .parse()
                .unwrap_or(1),
            neo4j_username: env::var("NEO4J_DB_USERNAME").expect("NEO4J_DB_USERNAME not set"),
            neo4j_password: env::var("NEO4J_PASSWORD").expect("NEO4J_PASSWORD not set"),
        }
    }

    pub fn neo4j_uri(&self) -> String {
        format!("bolt://{}:{}", self.neo4j_host, self.neo4j_port)
    }

    pub fn redis_uri(&self) -> String {
        format!("redis://{}:{}", self.redis_host, self.redis_port)
    }

    pub fn server_binding(&self) -> String {
        format!("{}:{}", self.server_host, self.server_port)
    }
}
