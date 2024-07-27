use crate::db::connectors::redis::get_redis_conn;
use log::debug;
use redis::{AsyncCommands, JsonAsyncCommands};
use serde::Serialize;
use std::error::Error;

/// Sets a value in Redis, supporting both JSON objects and boolean values.
///
/// This function stores JSON objects using RedisJSON and booleans as integers (0 or 1).
/// An optional expiration time can be set for both types.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis key.
/// * `key` - A string slice representing the key under which the value is stored.
/// * `value` - A reference to the value to be stored. If the value is a boolean, it will be stored as 0 or 1. For other types, it must implement `Serialize`.
/// * `path` - An optional string slice representing the JSON path where the value should be set for JSON objects. Defaults to the root path "$".
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
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let index_key = format!("{}:{}", prefix, key);

    match serde_json::to_value(value)? {
        serde_json::Value::Bool(boolean_value) => {
            set_boolean(&index_key, boolean_value, expiration).await?;
        }
        _ => {
            set_json(&index_key, value, path, expiration).await?;
        }
    }

    debug!(
        "Set key: {} with optional expiration: {:?}",
        index_key, expiration
    );
    Ok(())
}

async fn set_boolean(
    key: &str,
    value: bool,
    expiration: Option<i64>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut redis_conn = get_redis_conn().await?;

    let int_value = if value { 1 } else { 0 };
    if let Some(exp) = expiration {
        redis_conn.set_ex(key, int_value, exp as u64).await?;
    } else {
        redis_conn.set(key, int_value).await?;
    }
    Ok(())
}

async fn set_json<T: Serialize + Send + Sync>(
    key: &str,
    value: &T,
    path: Option<&str>,
    expiration: Option<i64>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut redis_conn = get_redis_conn().await?;

    let json_path = path.unwrap_or("$");
    redis_conn.json_set(key, json_path, value).await?;
    if let Some(exp) = expiration {
        redis_conn.expire(key, exp).await?;
    }
    Ok(())
}
