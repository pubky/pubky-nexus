use crate::types::DynError;
use deadpool_redis::{Config, Connection, Pool, Runtime};
use once_cell::sync::OnceCell;
use std::fmt;
use tracing::{debug, info};

pub struct RedisConnector {
    pool: Pool,
}

impl RedisConnector {
    /// Initialize and register the global Redis connector
    pub async fn init(redis_uri: &str) -> Result<(), DynError> {
        let redis_connector = RedisConnector::new_connection(redis_uri)
            .await
            .expect("Failed to connect to Redis");

        redis_connector.ping(redis_uri).await?;

        match REDIS_CONNECTOR.set(redis_connector) {
            Err(e) => debug!("RedisConnector was already set: {:?}", e),
            Ok(()) => info!("RedisConnector successfully set up on {}", redis_uri),
        }
        Ok(())
    }

    /// Creates a new RedisConnector instance by building a connection pool using the provided URI.
    async fn new_connection(uri: &str) -> Result<Self, DynError> {
        // Create the deadpool-redis configuration from the URI.
        let cfg = Config::from_url(uri.to_string());

        // Create the connection pool. We use the Tokio runtime.
        let pool = cfg.create_pool(Some(Runtime::Tokio1))?;
        Ok(Self { pool })
    }

    /// Returns a reference to the underlying connection pool.
    fn pool(&self) -> &Pool {
        &self.pool
    }

    /// Perform a health-check PING against the Redis server
    async fn ping(&self, redis_uri: &str) -> Result<(), DynError> {
        let redis_conn = self.pool.get().await;
        match redis_conn {
            Ok(_) => info!(
                "Redis health check PING succeeded; server at {} is reachable",
                redis_uri
            ),
            Err(_) => return Err(format!("Failed to PING to Redis at {redis_uri}").into()),
        }
        Ok(())
    }
}

impl fmt::Debug for RedisConnector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedisConnector")
            .field("pool", &"deadpool_redis::Pool")
            .finish()
    }
}

/// Global RedisConnector instance.
/// Make sure to initialize this once when your application starts.
pub static REDIS_CONNECTOR: OnceCell<RedisConnector> = OnceCell::new();

/// Retrieves a Redis connection from the pool.
pub async fn get_redis_conn() -> Result<Connection, DynError> {
    let connector = REDIS_CONNECTOR
        .get()
        .ok_or("RedisConnector not initialized")?;
    let conn = connector.pool().get().await?;
    Ok(conn)
}
