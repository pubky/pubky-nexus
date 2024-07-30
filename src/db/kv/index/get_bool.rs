use crate::db::connectors::redis::get_redis_conn;
use log::debug;
use redis::AsyncCommands;
use std::error::Error;

/// Retrieves a boolean value from Redis.
///
/// # Arguments
///
/// * `prefix` - A string slice that represents the prefix for the Redis key.
/// * `key` - A string slice that represents the key under which the value is stored.
///
/// # Returns
///
/// Returns an `Option` containing the retrieved boolean value if it exists, or `None` if it does not.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn _get_bool(
    prefix: &str,
    key: &str,
) -> Result<Option<bool>, Box<dyn Error + Send + Sync>> {
    let mut redis_conn = get_redis_conn().await?;
    let index_key = format!("{}:{}", prefix, key);

    if let Ok(indexed_value) = redis_conn.get::<_, i32>(&index_key).await {
        debug!(
            "Restored boolean key: {} with value: {}",
            index_key, indexed_value
        );
        let value = match indexed_value {
            1 => true,
            0 => false,
            _ => return Ok(None), // Invalid value in Redis
        };
        return Ok(Some(value));
    }

    Ok(None)
}
