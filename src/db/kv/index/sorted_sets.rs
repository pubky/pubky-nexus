use crate::db::connectors::redis::get_redis_conn;
use redis::AsyncCommands;
use std::error::Error;

pub enum SortOrder {
    Ascending,
    Descending,
}

pub enum ScoreAction {
    Increment(f64),
    Decrement(f64),
}

pub const SORTED_PREFIX: &str = "Sorted";

/// Checks if a member exists in a Redis sorted set and retrieves its score.
///
/// This function checks whether a specified member exists in a Redis sorted set
/// by retrieving its score using the `ZSCORE` command. If the member is present,
/// its score is returned; if it is not present, `None` is returned.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis key.
/// * `key` - A string slice representing the key under which the sorted set is stored.
/// * `member` - A string slice representing the member to check in the sorted set.
///
/// # Returns
///
/// Returns an `Option<isize>` containing the score of the member if it exists, or `None` if it does not.
pub async fn check_member(
    prefix: &str,
    key: &str,
    member: &str,
) -> Result<Option<isize>, Box<dyn Error + Send + Sync>> {
    let index_key = format!("{}:{}", prefix, key);
    let mut redis_conn = get_redis_conn().await?;
    // Use the ZSCORE command to check if the member exists in the sorted set
    let rank = redis_conn.zscore(index_key, member).await?;
    Ok(rank)
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

    let _: () = redis_conn.zadd_multiple(&index_key, items).await?;

    Ok(())
}

/// Updates the score of a member in a Redis sorted set.
///
/// This function modifies the score of a member in the specified Redis sorted set by incrementing or decrementing it
/// based on the provided `ScoreAction`.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis keys.
/// * `key` - A string slice representing the key under which the sorted set is stored.
/// * `member` - A string slice representing the member whose score will be updated.
/// * `score_mutation` - A `ScoreAction` that indicates whether to increment or decrement the score.
pub async fn put_score(
    prefix: &str,
    key: &str,
    member: &str,
    score_mutation: ScoreAction,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let index_key = format!("{}:{}", prefix, key);
    let mut redis_conn = get_redis_conn().await?;
    let value = match score_mutation {
        ScoreAction::Increment(val) => val,
        ScoreAction::Decrement(val) => -val,
    };
    let _: () = redis_conn.zincr(&index_key, member, value).await?;

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
    sorting: SortOrder,
) -> Result<Option<Vec<(String, f64)>>, Box<dyn Error + Send + Sync>> {
    let mut redis_conn = get_redis_conn().await?;
    let index_key = format!("{}:{}", prefix, key);

    let min_score = min_score.unwrap_or(f64::MIN);
    let max_score = max_score.unwrap_or(f64::MAX);
    let skip = skip.unwrap_or(0) as isize;
    let limit = limit.unwrap_or(1000) as isize;

    // ZRANGE with the WITHSCORES option retrieves both: the elements and their scores
    let elements: Vec<(String, f64)> = match sorting {
        SortOrder::Ascending => {
            redis_conn
                .zrangebyscore_limit_withscores(index_key, min_score, max_score, skip, limit)
                .await?
        }
        SortOrder::Descending => {
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
    let _: () = redis_conn.zrem(&index_key, items).await?;
    Ok(())
}

/// Removes elements from a Redis sorted set.
///
/// This function removes the specified elements from the Redis sorted set identified by the `prefix` and `key`.
/// If the sorted set does not exist, it will simply return without error.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis keys.
/// * `key` - A string slice representing the key under which the sorted set is stored.
/// * `values` - A slice of string slices representing the elements to be removed from the sorted set.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn del(
    prefix: &str,
    key: &str,
    values: &[&str],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if values.is_empty() {
        return Ok(());
    }

    let index_key = format!("{}:{}", prefix, key);
    let mut redis_conn = get_redis_conn().await?;

    // Remove the elements from the sorted set
    let _: () = redis_conn.zrem(index_key, values).await?;
    Ok(())
}
