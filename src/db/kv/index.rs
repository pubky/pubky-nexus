use crate::db::connectors::redis::get_redis_conn;
use log::debug;
use redis::{AsyncCommands, JsonAsyncCommands};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::error::Error;

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
    let index_key = format!("{}:{}", prefix, key);
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
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut redis_conn = get_redis_conn().await?;
    let index_key = format!("{}:{}", prefix, key);

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

/// Retrieves a range of values from Redis based on a pattern, with optional skip and limit for pagination.
///
/// # Arguments
///
/// * `prefix` - A string slice that represents the prefix for the Redis key.
/// * `pattern` - An optional string slice representing the pattern to match keys.
/// * `skip` - An optional number of keys to skip (for pagination).
/// * `limit` - An optional number of keys to return (for pagination).
///
/// # Returns
///
/// Returns a vector of deserialized values if they exist, or an empty vector if no matching keys are found.
///
/// # Note
/// The order of keys returned is not guaranteed to match the order in which they were inserted or any specific order.
/// This is due to the underlying behavior of the SCAN operation in Redis, which does not ensure a consistent order of keys.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn get_range<T: DeserializeOwned + Send + Sync>(
    prefix: &str,
    pattern: Option<&str>,
    skip: Option<usize>,
    limit: Option<usize>,
) -> Result<Vec<T>, Box<dyn Error + Send + Sync>> {
    let pattern = pattern.unwrap_or("*");
    let mut redis_conn = get_redis_conn().await?;
    let mut iter = redis_conn
        .scan_match::<String, String>(format!("{}{}", prefix, pattern))
        .await?;

    let mut keys_to_get = vec![];
    while let Some(key) = iter.next_item().await {
        keys_to_get.push(key);
    }

    let skip = skip.unwrap_or(0);
    let limit = limit.unwrap_or(keys_to_get.len());

    // Drop the iterator to release the mutable borrow on redis_conn
    drop(iter);

    let selected_keys: Vec<String> = keys_to_get.into_iter().skip(skip).take(limit).collect();
    let values: Vec<String> = redis_conn.mget(selected_keys).await?;

    let mut results = vec![];
    for value in values {
        let deserialized_value: T = serde_json::from_str(&value)?;
        results.push(deserialized_value);
    }

    Ok(results)
}

/// Sets a list of keys and their corresponding JSON values in Redis in a single operation.
///
/// This function uses multiple `SET` commands in a single request to set key-value pairs efficiently.
/// Each value must implement the `Serialize` trait to be converted into JSON format before storing.
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
/// ];
///
/// set_multiple::<MyValue>("prefix:", &data).await?;
/// ```
///
/// This example sets multiple key-value pairs with a common prefix in Redis.
pub async fn set_multiple<T: Serialize>(
    prefix: &str,
    data: &[(impl AsRef<str>, T)],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut redis_conn = get_redis_conn().await?;

    // Create a pipeline-like command sequence
    let mut cmd = redis::pipe();

    for (key, value) in data {
        let full_key = format!("{}{}", prefix, key.as_ref());
        let json_value = serde_json::to_string(&value)?;
        cmd.set(&full_key, json_value);
    }

    cmd.query_async(&mut redis_conn).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{setup, Config};
    use serde::{Deserialize, Serialize};
    use tokio;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct MyValue {
        field1: String,
        field2: i32,
    }

    #[tokio::test]
    async fn test_get_range() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let config = Config::from_env();
        setup(&config).await;

        let data = vec![
            (
                "key1",
                MyValue {
                    field1: "value1".to_string(),
                    field2: 10,
                },
            ),
            (
                "key2",
                MyValue {
                    field1: "value2".to_string(),
                    field2: 20,
                },
            ),
            (
                "key3",
                MyValue {
                    field1: "value3".to_string(),
                    field2: 30,
                },
            ),
        ];

        // Set values in Redis
        set_multiple::<MyValue>("test:", &data).await?;

        // Retrieve values using `get_range` with a specific pattern
        let result = get_range::<MyValue>("test:", Some("key*"), Some(0), Some(10)).await?;
        assert_eq!(result.len(), data.len());

        println!("{:?}", result);
        let expected_values: Vec<MyValue> = data.into_iter().map(|(_, v)| v).collect();

        for expected in expected_values.iter() {
            assert!(result.contains(expected));
        }

        Ok(())
    }
}
