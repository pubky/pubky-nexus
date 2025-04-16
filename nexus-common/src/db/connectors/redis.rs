use crate::types::DynError;
use deadpool_redis::{Config, Connection, Pool, Runtime};
use once_cell::sync::OnceCell;
use std::fmt;

pub struct RedisConnector {
    pool: Pool,
}

impl RedisConnector {
    /// Creates a new RedisConnector instance by building a connection pool using the provided URI.
    pub async fn new_connection(uri: &str) -> Result<Self, DynError> {
        // Create the deadpool-redis configuration from the URI.
        let cfg = Config::from_url(uri.to_string());

        // Create the connection pool. We use the Tokio runtime.
        let pool = cfg.create_pool(Some(Runtime::Tokio1))?;
        Ok(Self { pool })
    }

    /// Returns a reference to the underlying connection pool.
    pub fn pool(&self) -> &Pool {
        &self.pool
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
