use once_cell::sync::OnceCell;
pub use redis::AsyncCommands;
use redis::Client;
use std::fmt;

pub struct RedisConnector {
    client: OnceCell<Client>,
}

impl Default for RedisConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl RedisConnector {
    pub fn new() -> Self {
        Self {
            client: OnceCell::new(),
        }
    }

    pub async fn connect(&self, uri: &str) -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::open(uri)?;
        self.client
            .set(client)
            .map_err(|_| "Failed to set Redis client instance")?;
        Ok(())
    }

    pub async fn new_connection(uri: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let redis_connector = RedisConnector::new();
        redis_connector.connect(uri).await?;
        Ok(redis_connector)
    }

    pub fn client(&self) -> &Client {
        self.client.get().expect("Not connected to Redis")
    }
}

impl fmt::Debug for RedisConnector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedisConnector")
            .field("client", &"Redis client instance")
            .finish()
    }
}

pub static REDIS_CONNECTOR: OnceCell<RedisConnector> = OnceCell::new();
