use crate::db::connectors::redis::get_redis_conn;
use redis::AsyncCommands;
use std::error::Error;

/// Adds elements to a Redis sorted set.
///
/// This function adds elements to the specified Redis sorted set. If the set doesn't exist,
/// it creates a new sorted set.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis keys.
/// * `key` - A string slice representing the key under which the sorted set is stored.
/// * `values` - A slice of tuples where each tuple contains a reference to a string slice representing
///              the element and a f64 representing the score of the element.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn _put(
    prefix: &str,
    key: &str,
    values: &[(&str, f64)],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if values.is_empty() {
        return Ok(());
    }

    let index_key = format!("{}:{}", prefix, key);
    let mut redis_conn = get_redis_conn().await?;

    for (element, score) in values {
        redis_conn.zadd(&index_key, element, *score).await?;
    }

    Ok(())
}
