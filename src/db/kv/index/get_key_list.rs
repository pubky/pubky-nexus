use crate::db::connectors::redis::get_redis_conn;
use redis::AsyncCommands;
use std::error::Error;

/// Retrieves a keys from Redis at a specified pattern.
///
/// # Arguments
///
/// * `prefix` - A string slice that represents the prefix for the Redis key.
/// * `pattern` - A string slice that represents the key under which the value is stored.
/// * `path` - An optional string slice representing the JSON path from which the value should be retrieved. Defaults to the root path "$".
///
/// # Returns
///
/// Returns an `Option` containing the retrieved value if it exists, or `None` if it does not.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn get_key_list(
    prefix: &str,
    key: &str,
    _path: Option<&str>,
) -> Result<Option<Vec<String>>, Box<dyn Error + Send + Sync>> {
    let mut redis_conn = get_redis_conn().await?;
    let index_key = format!("{}:{}", prefix, key);

    // Use RedisJSON commands to get the value from the specified path
    let mut keys_iterator = redis_conn
        .scan_match::<String, String>(index_key.clone())
        .await?;

    let mut tags_keys: Vec<String> = vec![];

    while let Some(key) = keys_iterator.next_item().await {
        tags_keys.push(key);
    }

    Ok(Some(tags_keys))
}
