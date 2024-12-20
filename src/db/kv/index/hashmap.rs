use redis::AsyncCommands;

use crate::{db::connectors::redis::get_redis_conn, types::DynError};

/// puts multiple values into a hashmap in Redis
///
/// # Arguments
///
/// * `prefix` - A string slice that represents the prefix for the Redis key.
/// * `key` - A string slice that represents the key under which the value is stored.
/// * `values` - A vector of tuples containing the field and value to be stored.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn put_index_hashmap(
    prefix: &str,
    key: &str,
    values: &[(&str, &str)],
) -> Result<(), DynError> {
    let mut conn = get_redis_conn().await?;
    let index_key = format!("{}:{}", prefix, key);
    let _: () = conn.hset_multiple(index_key, values).await?;
    Ok(())
}

/// gets a value from a hashmap in Redis
///
/// # Arguments
///
/// * `prefix` - A string slice that represents the prefix for the Redis key.
/// * `key` - A string slice that represents the key under which the value is stored.
/// * `field` - A string slice that represents the field under which the value is stored.
///
/// # Errors
///
/// Returns an error if the operation fails.
///
/// # Returns
///
/// Returns an Option containing the value if it exists, or None if it does not.
pub async fn get_index_hashmap(
    prefix: &str,
    key: &str,
    field: &str,
) -> Result<Option<String>, DynError> {
    let mut conn = get_redis_conn().await?;
    let index_key = format!("{}:{}", prefix, key);
    let value: Option<String> = conn.hget(index_key, field).await?;
    Ok(value)
}

/// deletes a value from a hashmap in Redis
///
/// # Arguments
///
/// * `prefix` - A string slice that represents the prefix for the Redis key.
/// * `key` - A string slice that represents the key under which the value is stored.
/// * `field` - A string slice that represents the field under which the value is stored.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn del_index_hashmap(prefix: &str, key: &str, field: &str) -> Result<(), DynError> {
    let mut conn = get_redis_conn().await?;
    let index_key = format!("{}:{}", prefix, key);
    let _: () = conn.hdel(index_key, field).await?;
    Ok(())
}
