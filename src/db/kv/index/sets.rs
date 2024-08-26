use crate::db::connectors::redis::get_redis_conn;
use redis::AsyncCommands;
use std::error::Error;

/// Adds elements to a Redis set.
///
/// This function adds elements to the specified Redis set. If the set doesn't exist,
/// it creates a new set.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis keys.
/// * `key` - A string slice representing the key under which the set is stored.
/// * `values` - A slice of string slices representing the elements to be added to the set.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn put(
    prefix: &str,
    key: &str,
    values: &[&str],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if values.is_empty() {
        return Ok(());
    }
    let index_key = format!("{}:{}", prefix, key);
    let mut redis_conn = get_redis_conn().await?;
    redis_conn.sadd(index_key, values).await?;
    Ok(())
}

/// Retrieves a range of elements from a Redis set.
///
/// This function retrieves elements from a specified Redis set using a cursor-based approach.
/// The range is defined by `skip` and `limit` parameters, where `skip` indicates the number
/// of elements to skip and `limit` specifies the number of elements to retrieve.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis keys.
/// * `key` - A string slice representing the key under which the set is stored.
/// * `skip` - The number of elements to skip.
/// * `limit` - The number of elements to retrieve from the set after the skip.
///
/// # Returns
///
/// Returns a vector of strings containing the retrieved elements.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn get_range(
    prefix: &str,
    key: &str,
    skip: Option<usize>,
    limit: Option<usize>,
) -> Result<Option<Vec<String>>, Box<dyn Error + Send + Sync>> {
    let mut redis_conn = get_redis_conn().await?;

    let index_key = format!("{}:{}", prefix, key);
    let mut cursor = "0".to_string();
    let mut collected: Vec<String> = Vec::new();
    let skip = skip.unwrap_or(0);
    let limit = limit.unwrap_or(5);

    let mut skipped = 0;

    while collected.len() < limit {
        let result: (String, Vec<String>) = redis::cmd("SSCAN")
            .arg(&index_key)
            .arg(&cursor)
            .arg("COUNT")
            .arg(limit)
            .query_async(&mut redis_conn)
            .await?;

        let (new_cursor, items) = result;

        for item in items {
            if skipped < skip {
                skipped += 1;
                continue;
            }
            collected.push(item);
            if collected.len() >= limit {
                break;
            }
        }

        cursor = new_cursor;
        if cursor == "0" {
            break; // End of the set reached
        }
    }

    if collected.is_empty() {
        Ok(None)
    } else {
        Ok(Some(collected))
    }
}

/// Checks if a member exists in a Redis set and if the set exists.
///
/// This function checks if the specified `member` exists within the Redis set identified
/// by the combined `prefix` and `key`, and whether the set itself exists.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis keys.
/// * `key` - A string slice representing the key under which the set is stored.
/// * `member` - A string slice representing the member to check for existence in the set.
///
/// # Returns
///
/// Returns `Ok((true, true))` if the set exists and the member is in the set,
/// `Ok((true, false))` if the set exists but the member is not in the set,
/// `Ok((false, false))` if the set does not exist.
///
/// Returns an error if the operation fails, such as if the Redis connection is unavailable.
pub async fn check_set_member(
    prefix: &str,
    key: &str,
    member: &str,
) -> Result<(bool, bool), Box<dyn Error + Send + Sync>> {
    let mut redis_conn = get_redis_conn().await?;
    let index_key = format!("{}:{}", prefix, key);

    // Check if the set exists
    let set_exists: bool = redis_conn.exists(&index_key).await?;

    if set_exists {
        // Check if the member exists in the set
        let is_member: bool = redis_conn.sismember(&index_key, member).await?;
        Ok((true, is_member))
    } else {
        Ok((false, false))
    }
}

/// Retrieves multiple sets from Redis in a single call using a pipeline.
///
/// This asynchronous function fetches multiple sets from Redis based on the provided keys using a Redis pipeline.
/// It returns a vector of optional tuples, where each tuple contains a vector of elements from the corresponding set
/// and an integer representing the number of elements that were excluded if a limit was specified.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix to be prepended to each Redis key.
/// * `keys` - A slice of string slices representing the keys under which the sets are stored.
/// * `limit` - An optional `usize` specifying the maximum number of elements to retrieve from each set.
///   If `None`, all elements will be retrieved.
///
/// # Returns
///
/// Returns a `Result` containing:
/// * `Ok(Vec<Option<(Vec<String>, usize)>>)` - A vector where each element is an `Option` containing a tuple:
///     * `Some((Vec<String>, usize))` - The vector of elements from the set and the count of excluded elements.
///     * `None` - Indicates that the corresponding set does not exist.
/// * `Err` - An error if the Redis operation fails.
///
/// # Errors
///
/// Returns an error if the Redis connection fails or the pipeline query encounters an issue.

pub async fn get_multiple_sets(
    prefix: &str,
    keys: &[&str],
    limit: Option<usize>,
) -> Result<Vec<Option<(Vec<String>, usize)>>, Box<dyn Error + Send + Sync>> {
    let mut redis_conn = get_redis_conn().await?;

    // Create a Redis pipeline
    let mut pipe = redis::pipe();

    // Add each SMEMBERS command to the pipeline for all keys
    for key in keys {
        let index_key = format!("{}:{}", prefix, key);
        pipe.smembers(index_key);
    }

    // Execute the pipeline
    let results: Vec<Vec<String>> = pipe.query_async(&mut redis_conn).await?;

    let taggers_list = results
        .into_iter()
        .map(|set| {
            if set.is_empty() {
                None
            } else {
                let set_length = set.len();
                match limit {
                    Some(set_limit) if set_limit < set_length => {
                        let limited_set = set.into_iter().take(set_limit).collect();
                        Some((limited_set, set_length))
                    }
                    _ => Some((set, set_length)),
                }
            }
        })
        .collect();

    Ok(taggers_list)
}

/// Adds elements to multiple Redis sets using a pipeline.
///
/// This asynchronous function adds elements to multiple Redis sets, with the keys generated by concatenating the provided `prefix`, `common_key`, and each element of `index`.
/// Each set is uniquely identified by a combination of these keys, ensuring that each element in each set is unique.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis keys.
/// * `common_key` - A slice of string slices representing the common components of the Redis keys, which will be joined with colons (`:`).
/// * `index` - A slice of string slices representing the unique identifiers to append to the generated keys.
/// * `collections` - A slice of slices, where each inner slice contains elements to be added to the corresponding Redis set. The elements in each collection are added to the Redis set identified by the respective key from the `index`.
///
/// # Errors
///
/// This function will return an error if:
/// - The Redis connection cannot be established.
/// - The pipeline query fails to execute.
pub async fn put_multiple_sets(
    prefix: &str,
    common_key: &[&str],
    index: &[&str],
    collections: &[&[&str]],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut redis_conn = get_redis_conn().await?;
    let mut pipe = redis::pipe();

    for (i, key) in index.iter().enumerate() {
        let full_index = format!("{}:{}:{}", 
            &prefix,
            common_key.join(":"),
            key
        );
        if !collections[i].is_empty() {
            pipe.sadd(full_index, collections[i]);
        }
    }

    // Execute the pipeline
    pipe.query_async(&mut redis_conn).await?;
    Ok(())
}