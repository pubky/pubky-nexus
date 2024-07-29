use crate::db::connectors::redis::get_redis_conn;
use log::{debug, error};
use redis::Commands;
use redis::{RedisResult, Value};
use std::error::Error;

#[derive(PartialEq)]
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
    let mut redis_conn = get_redis_conn().await?;
    let limit = limit.unwrap_or(100);
    let skip = skip.unwrap_or(0);
    let full_pattern = format!("{}:{}", prefix, pattern.unwrap_or("*"));

    let mut result_keys = Vec::with_capacity(limit);
    let mut cursor = "0".to_string();
    let count = 1000;
    let mut skipped = 0;

    loop {
        let result: (String, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(&full_pattern)
            .arg("COUNT")
            .arg(count)
            .query_async(&mut redis_conn)
            .await?;

        let (new_cursor, keys) = result;

        for key in keys {
            if skipped < skip {
                skipped += 1;
                continue;
            }
            result_keys.push(key);
            if result_keys.len() >= limit {
                break;
            }
        }

        if result_keys.len() >= limit || new_cursor == "0" {
            break;
        }

        cursor = new_cursor;
    }

    if return_type == RangeReturnType::Keys {
        debug!("Restored keys: {:?}", result_keys);
        return Ok((Some(result_keys), None));
    }

    Ok((None, None))

    // all_keys.sort();

    // let skip = skip.unwrap_or(0);
    // let limit = limit.unwrap_or(all_keys.len());

    // let selected_keys: Vec<String> = all_keys.into_iter().skip(skip).take(limit).collect();

    // let fetch_values = async {
    //     let redis_values: Vec<Option<i32>> = redis_conn.mget(&selected_keys).await?;
    //     Ok::<Vec<bool>, Box<dyn Error + Send + Sync>>(
    //         redis_values
    //             .into_iter()
    //             .flatten()
    //             .map(|val| val != 0)
    //             .collect(),
    //     )
    // };

    // let (keys, values) = match return_type {
    //     RangeReturnType::Keys => (Some(selected_keys.clone()), None),
    //     RangeReturnType::Values => {
    //         let values = fetch_values.await?;
    //         (None, Some(values))
    //     }
    //     RangeReturnType::Both => {
    //         let values = fetch_values.await?;
    //         (Some(selected_keys), Some(values))
    //     }
    // };

    // debug!("Restored keys: {:?} with values: {:?}", keys, values);
    // Ok((keys, values))
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
//         let (_, values) = get_bool_range(
//             "test:",
//             Some("bool*"),
//             Some(0),
//             Some(10),
//             RangeReturnType::Values,
//         )
//         .await?;

//         // Ensure the returned result is not None
//         let result = values.ok_or("No values found")?;
//         assert_eq!(result.len(), data.len());

//         let expected_values: Vec<bool> = data.into_iter().map(|(_, v)| v).collect();
//         assert_eq!(result, expected_values);

//         Ok(())
//     }
// }
