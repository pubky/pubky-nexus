use crate::db::connectors::redis::get_redis_conn;
use redis::AsyncCommands;
use std::error::Error;

/// Adds elements to a Redis list.
///
/// This function appends elements to the specified Redis list. If the list doesn't exist,
/// it creates a new list.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis keys.
/// * `key` - A string slice representing the key under which the list is stored.
/// * `values` - A slice of string slices representing the elements to be added to the list.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn set_list(
    prefix: &str,
    key: &str,
    values: &[&str],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let index_key = format!("{}:{}", prefix, key);
    let mut redis_conn = get_redis_conn().await?;
    redis_conn.rpush(index_key, values).await?;
    Ok(())
}
