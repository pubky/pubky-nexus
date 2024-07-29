use crate::db::connectors::redis::get_redis_conn;
use redis::AsyncCommands;
use std::error::Error;

/// Retrieves a range of elements from a Redis list.
///
/// This function retrieves elements from a specified Redis list within a given range.
/// The range is defined by `skip` and `limit` parameters.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis keys.
/// * `key` - A string slice representing the key under which the list is stored.
/// * `skip` - The number of elements to skip from the beginning of the list.
/// * `limit` - The number of elements to retrieve from the list after the skip.
///
/// # Returns
///
/// Returns a vector of strings containing the retrieved elements.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn get_list_range(
    prefix: &str,
    key: &str,
    skip: Option<usize>,
    limit: Option<usize>,
) -> Result<Option<Vec<String>>, Box<dyn Error + Send + Sync>> {
    let mut redis_conn = get_redis_conn().await?;

    let index_key = format!("{}:{}", prefix, key);
    let skip = skip.unwrap_or(0);
    let limit = limit.unwrap_or(1000);

    let start = skip as isize;
    let end = start + (limit as isize) - 1;
    let result: Vec<String> = redis_conn.lrange(index_key, start, end).await?;
    match result.len() {
        0 => Ok(None),
        _ => Ok(Some(result)),
    }
}
