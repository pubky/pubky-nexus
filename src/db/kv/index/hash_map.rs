use crate::db::connectors::redis::get_redis_conn;
use crate::types::DynError;
use redis::AsyncCommands;

/// Inserts a value into a Redis hash map under the specified prefix, key, and field
/// * `prefix` - A string slice representing the prefix for the key. This is typically used to group related keys in Redis
/// * `key` - A string slice representing the main key in the hash map
/// * `field` - A string slice representing the field within the hash map where the value will be stored
/// * `value` - A `String` containing the value to be stored in the hash map
pub async fn put(prefix: &str, key: &str, field: &str, value: String) -> Result<(), DynError> {
    let index_key = format!("{}:{}", prefix, key);
    let mut redis_conn = get_redis_conn().await?;
    // HSETNX sets the field only if it does not exist. Returns 1 (true) or 0 (false).
    let _: bool = redis_conn.hset(&index_key, field, value).await?;
    Ok(())
}

/// Retrieves a value from a Redis hash map using the specified prefix, key, and field
/// # Arguments
/// * `prefix` - A string slice representing the prefix for the key. This is used to group related keys in Redis
/// * `key` - A string slice representing the main key in the hash map
/// * `field` - A string slice representing the field within the hash map from which the value will be retrieved
pub async fn get(prefix: &str, key: &str, field: &str) -> Result<Option<String>, DynError> {
    let index_key = format!("{}:{}", prefix, key);
    let mut redis_conn = get_redis_conn().await?;

    // HGET retrieves the value for the given field.
    let value: Option<String> = redis_conn.hget(&index_key, field).await?;
    Ok(value)
}

/// Deletes one or more fields from a Redis hash map under the specified prefix and key.
/// # Arguments
/// * `prefix` - A string slice representing the prefix for the key. This is used to group related keys in Redis
/// * `key` - A string slice representing the main key in the hash map
/// * `fields` - A slice of string slices representing the fields to be removed from the hash map
pub async fn _del(prefix: &str, key: &str, fields: &[&str]) -> Result<(), DynError> {
    if fields.is_empty() {
        return Ok(());
    }

    let index_key = format!("{}:{}", prefix, key);
    let mut redis_conn = get_redis_conn().await?;

    // The HDEL command is used to remove one or more fields from a hash.
    // It returns the number of fields that were removed
    let _removed_count: i32 = redis_conn.hdel(index_key, fields).await?;

    Ok(())
}
