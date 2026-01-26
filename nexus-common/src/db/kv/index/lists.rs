use crate::db::get_redis_conn;
use crate::db::kv::error::RedisResult;
use deadpool_redis::redis::AsyncCommands;

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
pub async fn put(prefix: &str, key: &str, values: &[&str]) -> RedisResult<()> {
    if values.is_empty() {
        return Ok(());
    }
    let index_key = format!("{prefix}:{key}");
    let mut redis_conn = get_redis_conn().await?;
    let _: () = redis_conn.rpush(index_key, values).await?;
    Ok(())
}

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
pub async fn get_range(
    prefix: &str,
    key: &str,
    skip: Option<usize>,
    limit: Option<usize>,
) -> RedisResult<Option<Vec<String>>> {
    let mut redis_conn = get_redis_conn().await?;

    let index_key = format!("{prefix}:{key}");
    let skip = skip.unwrap_or(0);
    let limit = limit.unwrap_or(usize::MAX);

    let start = skip as isize;
    let end = start + (limit as isize) - 1;
    let result: Vec<String> = redis_conn.lrange(index_key, start, end).await?;
    match result.len() {
        0 => Ok(None),
        _ => Ok(Some(result)),
    }
}
