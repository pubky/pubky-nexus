use crate::db::get_redis_conn;
use crate::db::kv::error::{RedisError, RedisResult};
use deadpool_redis::redis::Script;
use deadpool_redis::redis::{AsyncCommands, JsonAsyncCommands};
use serde::{de::DeserializeOwned, Serialize};
use tracing::{debug, trace};

#[derive(Clone, Debug)]
pub enum JsonAction {
    Increment(i64),
    Decrement(i64),
}

pub struct ValueRange {
    min: i64,
    max: i64,
}

impl Default for ValueRange {
    fn default() -> Self {
        Self {
            min: 0,
            max: u32::MAX as i64,
        }
    }
}

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
) -> RedisResult<()> {
    let index_key = format!("{prefix}:{key}");

    match to_json_value(value)? {
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

/// Modifies a numeric field in a Redis JSON object by either incrementing or decrementing it.
/// Uses LUA to ensure the value is never negative
///
/// This function uses the RedisJSON `JSON.NUMINCRBY` command to either increment or decrement a numeric field at a given path
/// based on the `JsonAction` provided.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis key.
/// * `key` - A string slice representing the Redis key where the JSON object is stored.
/// * `field` - A string slice representing the field to be modified in the JSON object.
/// * `action` - A `JsonAction` enum that specifies whether to increment or decrement the field.
///
/// # Errors
///
/// Returns an error if the operation fails or if the field does not exist or is not numeric.
pub async fn modify_json_field(
    prefix: &str,
    key: &str,
    field: &str,
    action: JsonAction,
    range: Option<ValueRange>,
) -> RedisResult<()> {
    let mut redis_conn = get_redis_conn().await?;
    let index_key = format!("{prefix}:{key}");
    let json_path = format!("$.{field}"); // Access the field using JSON path

    // Determine the action to take (increment or decrement)
    let amount = match action {
        JsonAction::Increment(value) => value,
        JsonAction::Decrement(value) => -value, // Negate the value for decrement
    };

    // Use default range if None provided
    let range = range.unwrap_or_default();

    // Lua script to safely increment/decrement without going below zero
    let script = Script::new(
        r#"
        local path = ARGV[1]
        local amount = tonumber(ARGV[2])
        local min_value = tonumber(ARGV[3])
        local max_value = tonumber(ARGV[4])
        local current = 0

        -- Fetch the current value as a JSON string
        local current_value = redis.call('JSON.GET', KEYS[1], path)

        if current_value ~= nil then
            -- Decode the JSON string into a Lua table
            local decoded = cjson.decode(current_value)

            if type(decoded) == 'table' then
                -- If the decoded value is an array, extract the first element
                if #decoded > 0 then
                    current = tonumber(decoded[1]) or 0
                end
            elseif type(decoded) == 'number' then
                -- If the decoded value is a number, use it directly
                current = decoded
            end
        end

        local new_value = current + amount

        -- Enforce min and max boundaries
        if new_value < min_value then
            new_value = min_value
        elseif new_value > max_value then
            new_value = max_value
        end

        -- Set the new value
        redis.call('JSON.SET', KEYS[1], path, new_value)
        return new_value
    "#,
    );

    debug!(
        "Modifiying field: {} in key: {} by {}",
        field, index_key, amount
    );

    let _: i64 = script
        .key(index_key)
        .arg(json_path)
        .arg(amount.to_string())
        .arg(range.min.to_string())
        .arg(range.max.to_string())
        .invoke_async(&mut redis_conn)
        .await?;

    Ok(())
}

/// Handles storing a boolean value in Redis with an optional expiration.
///
/// This function sets a key in Redis to either `1` or `0`, depending on the boolean value provided.
/// Optionally, an expiration time can be set for the key.
///
/// # Arguments
///
/// * `key` - A string slice representing the Redis key.
/// * `value` - A boolean value to store. If `true`, `1` is stored; if `false`, `0` is stored.
/// * `expiration` - An optional expiration time in seconds. If provided, the key will expire after this duration.
async fn handle_put_boolean(key: &str, value: bool, expiration: Option<i64>) -> RedisResult<()> {
    let mut redis_conn = get_redis_conn().await?;

    let int_value = if value { 1 } else { 0 };
    if let Some(exp) = expiration {
        let _: () = redis_conn.set_ex(key, int_value, exp as u64).await?;
    } else {
        let _: () = redis_conn.set(key, int_value).await?;
    }
    Ok(())
}

/// Handles storing a JSON object in Redis at a specified path, with optional expiration.
///
/// This function uses RedisJSON to store a JSON object under the provided key and path. An expiration time
/// can optionally be set for the key.
///
/// # Arguments
///
/// * `key` - A string slice representing the Redis key.
/// * `value` - A reference to the value to be stored, which must implement `Serialize`.
/// * `path` - An optional string slice representing the JSON path where the value should be set. Defaults to the root path "$".
/// * `expiration` - An optional expiration time in seconds. If provided, the key will expire after this duration.
async fn handle_put_json<T: Serialize + Send + Sync>(
    key: &str,
    value: &T,
    path: Option<&str>,
    expiration: Option<i64>,
) -> RedisResult<()> {
    let mut redis_conn = get_redis_conn().await?;

    let json_path = path.unwrap_or("$");
    let _: () = redis_conn.json_set(key, json_path, value).await?;
    if let Some(exp) = expiration {
        let _: () = redis_conn.expire(key, exp).await?;
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
) -> RedisResult<()> {
    let mut redis_conn = get_redis_conn().await?;

    // Create a pipeline-like command sequence
    let mut cmd = redis::pipe();

    for (key, value) in data {
        let full_key = format!("{}:{}", prefix, key.as_ref());

        // Check if the value is boolean
        match to_json_value(value)? {
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

    let _: () = cmd.query_async(&mut redis_conn).await?;
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
) -> RedisResult<Option<T>> {
    let mut redis_conn = get_redis_conn().await?;
    let index_key = format!("{prefix}:{key}");
    let json_path = path.unwrap_or("$").to_string(); // Ensure path is a String

    // Use RedisJSON commands to get the value from the specified path
    if let Ok(indexed_value) = redis_conn
        .json_get::<String, String, String>(index_key.clone(), json_path)
        .await
    {
        //debug!("Restored key: {} with value: {}", index_key, indexed_value);
        let value: Vec<T> = from_json_str(&indexed_value)?;
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
) -> RedisResult<Vec<Option<T>>> {
    let mut redis_conn = get_redis_conn().await?;
    let json_path = path.unwrap_or("$");

    // Generate full keys with prefix
    let full_keys: Vec<String> = keys
        .iter()
        .map(|key| format!("{}:{}", prefix, key.as_ref()))
        .collect();

    // Fetch values as Option<String> to handle missing keys
    let indexed_values: Vec<Option<String>> = redis_conn.json_get(&full_keys, json_path).await?;

    // Check if indexed_values is empty. That's an edge case 1 element and it was not found, redis does not return None.
    let results: Vec<Option<T>> = if indexed_values.is_empty() {
        (0..keys.len()).map(|_| None).collect()
    } else {
        deserialize_values(indexed_values)?
    };

    Ok(results)
}

// Helper function to deserialize JSON strings to Vec<Option<T>>
fn deserialize_values<T: DeserializeOwned>(
    values: Vec<Option<String>>,
) -> RedisResult<Vec<Option<T>>> {
    values
        .into_iter()
        .map(|value_str| match value_str {
            Some(value) => {
                let value: Vec<T> = from_json_str(&value)?;
                Ok(value.into_iter().next())
            }
            None => Ok(None),
        })
        .collect()
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
pub async fn _get_bool(prefix: &str, key: &str) -> RedisResult<Option<bool>> {
    let mut redis_conn = get_redis_conn().await?;
    let index_key = format!("{prefix}:{key}");

    if let Ok(indexed_value) = redis_conn.get::<_, i32>(&index_key).await {
        trace!(
            "Restored boolean key: {} with value: {}",
            index_key,
            indexed_value
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

/// Deletes multiple keys from Redis.
///
/// This function removes the specified keys from Redis. If a key does not exist, no error is returned.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis keys.
/// * `keys` - A slice of strings representing the keys to be deleted.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn del_multiple(prefix: &str, keys: &[impl AsRef<str>]) -> RedisResult<()> {
    let mut redis_conn = get_redis_conn().await?;

    // Generate full keys with prefix
    let full_keys: Vec<String> = keys
        .iter()
        .map(|key| format!("{}:{}", prefix, key.as_ref()))
        .collect();

    let _: () = redis_conn.del(full_keys).await?;
    Ok(())
}

fn to_json_value<T: Serialize>(value: &T) -> RedisResult<serde_json::Value> {
    serde_json::to_value(value).map_err(|e| RedisError::SerializationFailed(Box::new(e)))
}

fn from_json_str<T: DeserializeOwned>(s: &str) -> RedisResult<T> {
    serde_json::from_str(s).map_err(|e| RedisError::DeserializationFailed(Box::new(e)))
}
