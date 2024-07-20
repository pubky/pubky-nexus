use once_cell::sync::OnceCell;
use redis::aio::MultiplexedConnection;
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

/// Retrieves a Redis connection.
pub async fn get_redis_conn() -> Result<MultiplexedConnection, Box<dyn std::error::Error>> {
    let redis_client = REDIS_CONNECTOR
        .get()
        .ok_or("RedisConnector not initialized")?
        .client();
    let redis_conn = redis_client.get_multiplexed_async_connection().await?;
    Ok(redis_conn)
}

pub static REDIS_CONNECTOR: OnceCell<RedisConnector> = OnceCell::new();
