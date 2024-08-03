use crate::db::connectors::redis::get_redis_conn;
use log::debug;
use redis::{AsyncCommands, JsonAsyncCommands};
use serde::{de::DeserializeOwned, Serialize};
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
pub async fn put<T: Serialize + Send + Sync>(
    prefix: &str,
    key: &str,
    value: &T,
    path: Option<&str>,
    expiration: Option<i64>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let index_key = format!("{}:{}", prefix, key);

    match serde_json::to_value(value)? {
        serde_json::Value::Bool(boolean_value) => {
            handle_put_boolean(&index_key, boolean_value, expiration).await?;
        }
        _ => {
            handle_put_json(&index_key, value, path, expiration).await?;
        }
    }

    debug!(
        "Set key: {} with optional expiration: {:?}",
        index_key, expiration
    );
    Ok(())
}

async fn handle_put_boolean(
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

async fn handle_put_json<T: Serialize + Send + Sync>(
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

/// Sets a list of keys and their corresponding values in Redis in a single operation.
///
/// This function handles both JSON and boolean values, using RedisJSON for JSON data and storing booleans
/// as integers. It uses multiple commands in a single request to set key-value pairs efficiently.
/// Each value must implement the `Serialize` trait for JSON or be a boolean.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis keys.
/// * `data` - A slice of tuples where each tuple contains a key as a string slice and a value that implements `Serialize`.
///
/// # Returns
///
/// Returns a `Result` indicating success or an error if the operation fails.
///
/// # Errors
///
/// This function will return an error if there are issues connecting to Redis, or if serialization of the values fails.
///
/// # Examples
///
/// ```ignore
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct MyValue {
///     field1: String,
///     field2: i32,
/// }
///
/// let data = vec![
///     ("key1", MyValue { field1: "value1".to_string(), field2: 10 }),
///     ("key2", MyValue { field1: "value2".to_string(), field2: 20 }),
///     ("key3", true), // boolean value
/// ];
///
/// put_multiple::<MyValue>("prefix:", &data).await?;
/// ```
///
/// This example sets multiple key-value pairs with a common prefix in Redis.
pub async fn put_multiple<T: Serialize>(
    prefix: &str,
    data: &[(impl AsRef<str>, T)],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut redis_conn = get_redis_conn().await?;

    // Create a pipeline-like command sequence
    let mut cmd = redis::pipe();

    for (key, value) in data {
        let full_key = format!("{}:{}", prefix, key.as_ref());

        // Check if the value is boolean
        match serde_json::to_value(value)? {
            serde_json::Value::Bool(boolean_value) => {
                let int_value = if boolean_value { 1 } else { 0 };
                cmd.set(&full_key, int_value);
            }
            _ => {
                // Handle other values as JSON
                cmd.json_set(&full_key, "$", value)?;
            }
        }
    }

    cmd.query_async(&mut redis_conn).await?;
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
) -> Result<Option<T>, Box<dyn Error + Send + Sync>> {
    let mut redis_conn = get_redis_conn().await?;
    let index_key = format!("{}:{}", prefix, key);
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

/// Retrieves a list of JSON values from Redis based on a list of keys.
///
/// This function fetches JSON objects from Redis using RedisJSON based on the provided keys.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis keys.
/// * `keys` - A slice of strings representing the keys under which the values are stored.
/// * `path` - An optional string slice representing the JSON path from which the value should be retrieved. Defaults to the root path "$".
///
/// # Returns
///
/// Returns a vector of optional values corresponding to the provided keys.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn get_multiple<T: DeserializeOwned + Send + Sync>(
    prefix: &str,
    keys: &[impl AsRef<str>],
    path: Option<&str>,
) -> Result<Vec<Option<T>>, Box<dyn Error + Send + Sync>> {
    let mut redis_conn = get_redis_conn().await?;
    let json_path = path.unwrap_or("$");

    // Generate full keys with prefix
    let full_keys: Vec<String> = keys
        .iter()
        .map(|key| format!("{}:{}", prefix, key.as_ref()))
        .collect();

    let indexed_values: Vec<String> = redis_conn.json_get(&full_keys, json_path).await?;

    let mut results = Vec::with_capacity(indexed_values.len());

    for value in indexed_values {
        if value.is_empty() {
            results.push(None);
        } else {
            let deserialized_value: T = serde_json::from_str(&value)?;
            results.push(Some(deserialized_value));
        }
    }

    Ok(results)
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
) -> Result<Option<bool>, Box<dyn Error + Send + Sync>> {
    let mut redis_conn = get_redis_conn().await?;
    let index_key = format!("{}:{}", prefix, key);

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
