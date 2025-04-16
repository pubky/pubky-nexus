use crate::types::DynError;
use once_cell::sync::OnceCell;
use redis::aio::MultiplexedConnection;
use redis::Client;
use std::fmt;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct RedisConnector {
    client: OnceCell<Client>,
    // Wrap the multiplexed connection in a Mutex for mutable access.
    connection: OnceCell<Arc<Mutex<MultiplexedConnection>>>,
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
            connection: OnceCell::new(),
        }
    }

    pub async fn connect(&self, uri: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Create and store the Redis client.
        let client = Client::open(uri)?;
        self.client
            .set(client)
            .map_err(|_| "Failed to set Redis client instance")?;

        // Establish a multiplexed async connection,
        // then wrap it in a Mutex and store it in an Arc.
        let conn = self
            .client
            .get()
            .unwrap()
            .get_multiplexed_async_connection()
            .await?;
        self.connection
            .set(Arc::new(Mutex::new(conn)))
            .map_err(|_| "Failed to set multiplexed connection")?;

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

/// Retrieves the shared multiplexed Redis connection.
/// Returns an Arc of a Tokio Mutex guarding the connection.
pub async fn get_redis_conn() -> Result<Arc<Mutex<MultiplexedConnection>>, DynError> {
    let connector = REDIS_CONNECTOR
        .get()
        .ok_or("RedisConnector not initialized")?;
    let conn = connector
        .connection
        .get()
        .cloned()
        .ok_or("MultiplexedConnection not set")?;
    Ok(conn)
}

pub static REDIS_CONNECTOR: OnceCell<RedisConnector> = OnceCell::new();
