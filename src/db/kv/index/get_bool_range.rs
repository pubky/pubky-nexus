use crate::db::connectors::redis::get_redis_conn;
use redis::AsyncCommands;
use std::error::Error;

/// Retrieves a range of boolean values from Redis based on a pattern, with optional skip and limit for pagination.
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
/// Returns a vector of boolean values if they exist, or an empty vector if no matching keys are found.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn get_bool_range(
    prefix: &str,
    pattern: Option<&str>,
    skip: Option<usize>,
    limit: Option<usize>,
) -> Result<Vec<bool>, Box<dyn Error + Send + Sync>> {
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

    // Fetch the values using a single command
    let values: Vec<Option<i32>> = redis_conn.mget(&selected_keys).await?;

    let results: Vec<bool> = values
        .into_iter()
        .filter_map(|opt| opt)
        .map(|val| val != 0)
        .collect();

    Ok(results)
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::{db::kv::index::set_multiple, setup, Config};
//     use tokio;

//     #[tokio::test]
//     async fn test_get_bool_range() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//         let config = Config::from_env();
//         setup(&config).await;

//         let data = vec![("bool1", true), ("bool2", false), ("bool3", true)];

//         // Set boolean values in Redis
//         set_multiple::<bool>("test:", &data).await?;

//         // Retrieve boolean values using `get_bool_range` with a specific pattern
//         let result = get_bool_range("test:", Some("bool*"), Some(0), Some(10)).await?;
//         assert_eq!(result.len(), data.len());

//         let expected_values: Vec<bool> = data.into_iter().map(|(_, v)| v).collect();
//         assert_eq!(result, expected_values);

//         Ok(())
//     }
// }
