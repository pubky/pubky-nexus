use crate::db::connectors::redis::get_redis_conn;
use log::debug;
use redis::{AsyncCommands, JsonAsyncCommands};
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Sets a JSON value in Redis at a specified path with an optional expiration time.
///
/// # Arguments
///
/// * `prefix` - A string slice that represents the prefix for the Redis key.
/// * `key` - A string slice that represents the key under which the value is stored.
/// * `value` - A reference to the value to be stored, which must implement `Serialize`, `Send`, and `Sync`.
/// * `path` - An optional string slice representing the JSON path where the value should be set. Defaults to the root path "$".
/// * `expiration` - An optional expiration time in seconds. If provided, the key will expire after this duration.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn set<T: Serialize + Send + Sync>(
    prefix: &str,
    key: &str,
    value: &T,
    path: Option<&str>,
    expiration: Option<i64>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut redis_conn = get_redis_conn().await?;
    let index_key = format!("{}{}", prefix, key);
    let json_path = path.unwrap_or("$");

    // Use RedisJSON commands to set the value at the specified path
    redis_conn.json_set(&index_key, json_path, value).await?;

    if let Some(exp) = expiration {
        redis_conn.expire(&index_key, exp).await?;
        debug!("Indexed key: {} with expiration: {}", index_key, exp);
    } else {
        debug!("Indexed key: {} with no expiration", index_key);
    }

    Ok(())
}

/// Retrieves a JSON value from Redis at a specified path.
///
/// # Arguments
///
/// * `prefix` - A string slice that represents the prefix for the Redis key.
/// * `key` - A string slice that represents the key under which the value is stored.
/// * `path` - An optional string slice representing the JSON path from which the value should be retrieved. Defaults to the root path "$".
///
/// # Returns
///
/// Returns an `Option` containing the retrieved value if it exists, or `None` if it does not.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn get<T: DeserializeOwned + Send + Sync>(
    prefix: &str,
    key: &str,
    path: Option<&str>,
) -> Result<Option<T>, Box<dyn std::error::Error + Send + Sync>> {
    let mut redis_conn = get_redis_conn().await?;
    let index_key = format!("{}{}", prefix, key);
    let json_path = path.unwrap_or("$").to_string(); // Ensure path is a String

    // Use RedisJSON commands to get the value from the specified path
    if let Ok(indexed_value) = redis_conn
        .json_get::<String, String, String>(index_key.clone(), json_path)
        .await
    {
        debug!("Restored key: {} with value: {}", index_key, indexed_value);
        let value: Vec<T> = serde_json::from_str(&indexed_value)?;
        return Ok(value.into_iter().next()); // Extract the first element from the Vec
    }

    Ok(None)
}

/// NOT CURRENTLY BEING USED, BUT POTENTIALLY USEFUL
///
/// Sets a boolean value in Redis with an optional expiration time.
///
/// # Arguments
///
/// * `prefix` - A string slice that represents the prefix for the Redis key.
/// * `key` - A string slice that represents the key under which the value is stored.
/// * `value` - A boolean value to be stored.
/// * `expiration` - An optional expiration time in seconds. If provided, the key will expire after this duration.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn _set_bool(
    prefix: &str,
    key: &str,
    value: bool,
    expiration: Option<u64>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut redis_conn = get_redis_conn().await?;
    let index_key = format!("{}{}", prefix, key);

    let int_value = if value { 1 } else { 0 };

    match expiration {
        Some(exp) => {
            redis_conn.set_ex(&index_key, int_value, exp).await?;
            debug!(
                "Indexed boolean key: {} with expiration: {}",
                index_key, exp
            );
        }
        None => {
            redis_conn.set(&index_key, int_value).await?;
            debug!("Indexed boolean key: {} with no expiration", index_key);
        }
    }

    Ok(())
}

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
) -> Result<Option<bool>, Box<dyn std::error::Error + Send + Sync>> {
    let mut redis_conn = get_redis_conn().await?;
    let index_key = format!("{}{}", prefix, key);

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
