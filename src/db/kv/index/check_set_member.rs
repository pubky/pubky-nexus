use crate::db::connectors::redis::get_redis_conn;
use redis::AsyncCommands;
use std::error::Error;

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
