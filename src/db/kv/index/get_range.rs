use crate::db::connectors::redis::get_redis_conn;
use log::debug;
use redis::AsyncCommands;
use serde::de::DeserializeOwned;
use std::error::Error;

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

    // Sort keys alphanumerically
    keys_to_get.sort();

    let skip = skip.unwrap_or(0);
    let limit = limit.unwrap_or(keys_to_get.len());

    // Drop the iterator to release the mutable borrow on redis_conn
    drop(iter);

    let selected_keys: Vec<String> = keys_to_get.into_iter().skip(skip).take(limit).collect();

    // Using a pipeline to get multiple JSON values
    let mut pipeline = redis::pipe();
    for key in &selected_keys {
        pipeline.cmd("JSON.GET").arg(key).arg("$");
    }

    let json_values: Vec<Option<String>> = pipeline.query_async(&mut redis_conn).await?;

    let mut results = Vec::with_capacity(json_values.len());
    for json_value in json_values.into_iter().flatten() {
        let mut deserialized_values: Vec<T> = serde_json::from_str(&json_value)?;
        if let Some(value) = deserialized_values.pop() {
            results.push(value);
        }
    }

    debug!("Restored keys: {:?}", selected_keys);
    Ok(results)
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::{db::kv::index::set_multiple::set_multiple, setup, Config};
//     use serde::{Deserialize, Serialize};
//     use tokio;

//     #[derive(Serialize, Deserialize, Debug, PartialEq)]
//     struct MyValue {
//         field1: String,
//         field2: i32,
//     }

//     #[tokio::test]
//     async fn test_get_range() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//         let config = Config::from_env();
//         setup(&config).await;

//         let data = vec![
//             (
//                 "key1",
//                 MyValue {
//                     field1: "value1".to_string(),
//                     field2: 10,
//                 },
//             ),
//             (
//                 "key2",
//                 MyValue {
//                     field1: "value2".to_string(),
//                     field2: 20,
//                 },
//             ),
//             (
//                 "key3",
//                 MyValue {
//                     field1: "value3".to_string(),
//                     field2: 30,
//                 },
//             ),
//         ];

//         // Set values in Redis
//         set_multiple::<MyValue>("test:", &data).await?;

//         // Retrieve values using `get_range` with a specific pattern
//         let result = get_range::<MyValue>("test:", Some("key*"), Some(0), Some(10)).await?;
//         assert_eq!(result.len(), data.len());

//         let expected_values: Vec<MyValue> = data.into_iter().map(|(_, v)| v).collect();
//         assert_eq!(result, expected_values);

//         Ok(())
//     }
// }
