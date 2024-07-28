use crate::db::connectors::redis::get_redis_conn;
use log::debug;
use redis::AsyncCommands;
use std::error::Error;

pub enum RangeReturnType {
    #[allow(dead_code)]
    Keys,
    Values,
    #[allow(dead_code)]
    Both,
}

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
    return_type: RangeReturnType,
) -> Result<(Option<Vec<String>>, Option<Vec<bool>>), Box<dyn Error + Send + Sync>> {
    let pattern = pattern.unwrap_or("*");
    let mut redis_conn = get_redis_conn().await?;

    // TODO SCAN with count 1 and iter is extremely ineficient. Grows fast in time as number of keys grows.
    let mut iter = redis_conn
        .scan_match::<String, String>(format!("{}:{}", prefix, pattern))
        .await?;

    let mut keys_to_get = vec![];
    while let Some(key) = iter.next_item().await {
        keys_to_get.push(key);
    }

    // Drop the iterator to release the mutable borrow on redis_conn
    drop(iter);

    // Sort keys alphanumerically
    keys_to_get.sort();

    let skip = skip.unwrap_or(0);
    let limit = limit.unwrap_or(keys_to_get.len());

    let selected_keys: Vec<String> = keys_to_get.into_iter().skip(skip).take(limit).collect();

    let fetch_values = async {
        let redis_values: Vec<Option<i32>> = redis_conn.mget(&selected_keys).await?;
        Ok::<Vec<bool>, Box<dyn Error + Send + Sync>>(
            redis_values
                .into_iter()
                .flatten()
                .map(|val| val != 0)
                .collect(),
        )
    };

    let (keys, values) = match return_type {
        RangeReturnType::Keys => (Some(selected_keys.clone()), None),
        RangeReturnType::Values => {
            let values = fetch_values.await?;
            (None, Some(values))
        }
        RangeReturnType::Both => {
            let values = fetch_values.await?;
            (Some(selected_keys), Some(values))
        }
    };

    debug!("Restored keys: {:?} with values: {:?}", keys, values);
    Ok((keys, values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::kv::index::set_multiple, setup, Config};
    use tokio;

    #[tokio::test]
    async fn test_get_bool_range() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let config = Config::from_env();
        setup(&config).await;

        let data = vec![("bool1", true), ("bool2", false), ("bool3", true)];

        // Set boolean values in Redis
        set_multiple::<bool>("test:", &data).await?;

        // Retrieve boolean values using `get_bool_range` with a specific pattern
        let (_, values) = get_bool_range(
            "test:",
            Some("bool*"),
            Some(0),
            Some(10),
            RangeReturnType::Values,
        )
        .await?;

        // Ensure the returned result is not None
        let result = values.ok_or("No values found")?;
        assert_eq!(result.len(), data.len());

        let expected_values: Vec<bool> = data.into_iter().map(|(_, v)| v).collect();
        assert_eq!(result, expected_values);

        Ok(())
    }
}
