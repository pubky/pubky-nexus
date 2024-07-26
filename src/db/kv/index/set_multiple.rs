use crate::db::connectors::redis::get_redis_conn;
use serde::Serialize;
use std::error::Error;

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
