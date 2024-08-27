use crate::db::connectors::redis::get_redis_conn;
use redis::AsyncCommands;
use std::error::Error;

pub enum Sorting {
    Ascending,
    Descending,
}
/// Adds elements to a Redis sorted set.
///
/// This function adds elements to the specified Redis sorted set. If the set doesn't exist,
/// it creates a new sorted set.
///
/// # Argumentsf64
///
/// * `prefix` - A string slice representing the prefix for the Redis keys.
/// * `key` - A string slice representing the key under which the sorted set is stored.
/// * `values` - A slice of tuples where each tuple contains a reference to a string slice representing
///              the element and a f64 representing the score of the element.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn put(
    prefix: &str,
    key: &str,
    items: &[(f64, &str)],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if items.is_empty() {
        return Ok(());
    }

    let index_key = format!("{}:{}", prefix, key);
    let mut redis_conn = get_redis_conn().await?;

    redis_conn.zadd_multiple(&index_key, items).await?;

    Ok(())
}

/// Retrieves a range of elements from a Redis sorted set.
///
/// This function retrieves elements from a specified Redis sorted set based on a score range.
/// The range is defined by `min_score` and `max_score` parameters, where `min_score` and `max_score`
/// specify the inclusive lower and upper bounds of the scores.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis keys.
/// * `key` - A string slice representing the key under which the sorted set is stored.
/// * `min_score` - The minimum score for the range (inclusive).
/// * `max_score` - The maximum score for the range (inclusive).
/// * `limit` - The maximum number of elements to retrieve.
/// * `sorting` - The sorting order (ascending or descending).
///
/// # Returns
///
/// Returns a vector of tuples containing the elements and their scores.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn get_range(
    prefix: &str,
    key: &str,
    min_score: Option<f64>,
    max_score: Option<f64>,
    skip: Option<usize>,
    limit: Option<usize>,
    sorting: Sorting,
) -> Result<Option<Vec<(String, f64)>>, Box<dyn Error + Send + Sync>> {
    let mut redis_conn = get_redis_conn().await?;
    let index_key = format!("{}:{}", prefix, key);

    let min_score = min_score.unwrap_or(f64::MIN);
    let max_score = max_score.unwrap_or(f64::MAX);
    let skip = skip.unwrap_or(0) as isize;
    let limit = limit.unwrap_or(1000) as isize;

    // ZRANGE with the WITHSCORES option retrieves both: the elements and their scores
    let elements: Vec<(String, f64)> = match sorting {
        Sorting::Ascending => {
            redis_conn
                .zrangebyscore_limit_withscores(index_key, min_score, max_score, skip, limit)
                .await?
        }
        Sorting::Descending => {
            redis_conn
                .zrevrangebyscore_limit_withscores(index_key, max_score, min_score, skip, limit)
                .await?
        }
    };

    match elements.len() {
        0 => Ok(None),
        _ => Ok(Some(elements)),
    }
}

/// Performs a lexicographical range search on the Redis sorted set.
///
/// # Arguments
///
/// * `min` - The minimum lexicographical bound (inclusive).
/// * `max` - The maximum lexicographical bound (exclusive).
/// * `limit` - The maximum number of elements to retrieve.
pub async fn get_lex_range(
    prefix: &str,
    key: &str,
    min: &str,
    max: &str,
    skip: Option<usize>,
    limit: Option<usize>,
) -> Result<Option<Vec<String>>, Box<dyn Error + Send + Sync>> {
    let mut redis_conn = get_redis_conn().await?;
    let index_key = format!("{}:{}", prefix, key);
    let skip = skip.unwrap_or(0) as isize;
    let limit = limit.unwrap_or(1000) as isize;

    let elements: Vec<String> = redis_conn
        .zrangebylex_limit(index_key, min, max, skip, limit)
        .await?;

    match elements.len() {
        0 => Ok(None),
        _ => Ok(Some(elements)),
    }
}

/// Removes elements from the Redis sorted set.
///
/// # Arguments
///
/// * `items` - A slice of elements to remove.
pub async fn _remove(
    prefix: &str,
    key: &str,
    items: &[&str],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if items.is_empty() {
        return Ok(());
    }

    let index_key = format!("{}:{}", prefix, key);
    let mut redis_conn = get_redis_conn().await?;
    redis_conn.zrem(&index_key, items).await?;
    Ok(())
}
