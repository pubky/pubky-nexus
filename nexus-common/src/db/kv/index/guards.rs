use crate::db::get_redis_conn;
use crate::db::kv::RedisResult;

/// Attempts to acquire a guard key using `SET key 1 NX EX ttl`.
///
/// The key is only written if it does not already exist (SETNX semantics), so
/// concurrent or retried callers observe exactly one successful acquisition
/// until the key is released or its TTL expires. Unlike regular index entries,
/// a guard key is never written by read-through cache population, which makes
/// it a reliable "already ran" marker for non-idempotent side effects.
///
/// # Arguments
///
/// * `key` - The full Redis key for the guard (no prefix is added).
/// * `ttl_secs` - The TTL (in seconds) after which the guard expires on its own.
///
/// # Returns
///
/// Returns `Ok(true)` if the guard was acquired (the key did not exist),
/// `Ok(false)` if the guard is already held.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn try_acquire(key: &str, ttl_secs: u64) -> RedisResult<bool> {
    let mut redis_conn = get_redis_conn().await?;

    // SET with NX returns "OK" when the key was set, nil when it already exists.
    let outcome: Option<String> = redis::cmd("SET")
        .arg(key)
        .arg(1)
        .arg("NX")
        .arg("EX")
        .arg(ttl_secs)
        .query_async(&mut redis_conn)
        .await?;

    Ok(outcome.is_some())
}

/// Releases a guard key previously acquired with [`try_acquire`].
///
/// Deleting a non-existent key is a no-op, so releasing twice is safe.
///
/// # Arguments
///
/// * `key` - The full Redis key for the guard (no prefix is added).
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn release(key: &str) -> RedisResult<()> {
    let mut redis_conn = get_redis_conn().await?;

    let _: () = redis::cmd("DEL")
        .arg(key)
        .query_async(&mut redis_conn)
        .await?;
    Ok(())
}
