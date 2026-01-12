use crate::db::get_redis_conn;
use crate::types::DynError;
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
pub async fn put(prefix: &str, key: &str, values: &[&str]) -> Result<(), DynError> {
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
) -> Result<Option<Vec<String>>, DynError> {
    let mut redis_conn = get_redis_conn().await?;

    let index_key = format!("{prefix}:{key}");
    let skip = skip.unwrap_or(0);

    // Cap skip at isize::MAX to prevent overflow when casting
    let start = skip.min(isize::MAX as usize) as isize;

    // Calculate end index, handling potential overflow.
    // Redis LRANGE uses -1 to mean "to the end of the list".
    let end = match limit {
        Some(0) => return Ok(None),
        Some(lim) => {
            // Cap limit to prevent overflow when casting to isize
            let lim_capped = lim.min(isize::MAX as usize) as isize;
            // Use saturating arithmetic: start + (limit - 1)
            // Subtract 1 because lrange is inclusive on both ends
            start.saturating_add(lim_capped - 1)
        }
        None => -1,
    };

    let result: Vec<String> = redis_conn.lrange(index_key, start, end).await?;
    match result.len() {
        0 => Ok(None),
        _ => Ok(Some(result)),
    }
}
