use crate::db::get_redis_conn;
use crate::db::kv::RedisResult;
use deadpool_redis::redis::AsyncCommands;

/// Builds the HASH key. An empty `key` names the HASH by its prefix alone, so a
/// model-wide HASH such as `User:Epoch` gets no trailing `:`.
fn index_key(prefix: &str, key: &str) -> String {
    if key.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}:{key}")
    }
}

/// Reads one field of a Redis HASH.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis keys.
/// * `key` - A string slice representing the key under which the HASH is stored.
/// * `field` - The field to read.
///
/// # Returns
///
/// Returns `Ok(None)` if the HASH or the field does not exist.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn get(prefix: &str, key: &str, field: &str) -> RedisResult<Option<String>> {
    let mut redis_conn = get_redis_conn().await?;
    let value = redis_conn.hget(index_key(prefix, key), field).await?;
    Ok(value)
}

/// Sets several fields of a Redis HASH in one `HSET`, creating the HASH if it doesn't exist.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis keys.
/// * `key` - A string slice representing the key under which the HASH is stored.
/// * `entries` - A slice of `(field, value)` pairs to store.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn put(prefix: &str, key: &str, entries: &[(&str, &str)]) -> RedisResult<()> {
    if entries.is_empty() {
        return Ok(());
    }
    let mut redis_conn = get_redis_conn().await?;
    let _: () = redis_conn
        .hset_multiple(index_key(prefix, key), entries)
        .await?;
    Ok(())
}

/// Removes fields from a Redis HASH. Missing fields are ignored.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis keys.
/// * `key` - A string slice representing the key under which the HASH is stored.
/// * `fields` - A slice of string slices representing the fields to remove.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn del(prefix: &str, key: &str, fields: &[&str]) -> RedisResult<()> {
    if fields.is_empty() {
        return Ok(());
    }
    let mut redis_conn = get_redis_conn().await?;
    let _: () = redis_conn.hdel(index_key(prefix, key), fields).await?;
    Ok(())
}
