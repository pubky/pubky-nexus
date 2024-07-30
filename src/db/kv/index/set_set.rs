use crate::db::connectors::redis::get_redis_conn;
use redis::AsyncCommands;
use std::error::Error;

/// Adds elements to a Redis set.
///
/// This function adds elements to the specified Redis set. If the set doesn't exist,
/// it creates a new set.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis keys.
/// * `key` - A string slice representing the key under which the set is stored.
/// * `values` - A slice of string slices representing the elements to be added to the set.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn set_set(
    prefix: &str,
    key: &str,
    values: &[&str],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if values.is_empty() {
        return Ok(());
    }
    let index_key = format!("{}:{}", prefix, key);
    let mut redis_conn = get_redis_conn().await?;
    redis_conn.sadd(index_key, values).await?;
    Ok(())
}
