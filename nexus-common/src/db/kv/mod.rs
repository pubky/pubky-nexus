mod error;
mod flush;
mod index;
mod last_save;
mod traits;

pub use error::{RedisError, RedisResult};
pub use flush::clear_redis;
pub use index::json::JsonAction;
pub use index::sets;
pub use index::sorted_sets::{ScoreAction, SortOrder};
pub use last_save::get_last_rdb_save_time;
pub use traits::RedisOps;

/// Deletes all Redis keys matching the given pattern (e.g. `"Muted:*"`).
/// Uses SCAN to avoid blocking Redis on large keyspaces.
pub async fn delete_keys_by_pattern(pattern: &str) -> RedisResult<usize> {
    use crate::db::get_redis_conn;

    let mut redis_conn = get_redis_conn().await?;
    let mut cursor: u64 = 0;
    let mut total_deleted = 0;

    loop {
        let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(100)
            .query_async(&mut redis_conn)
            .await?;

        if !keys.is_empty() {
            let count = keys.len();
            redis::cmd("DEL")
                .arg(&keys)
                .query_async::<()>(&mut redis_conn)
                .await?;
            total_deleted += count;
        }

        cursor = next_cursor;
        if cursor == 0 {
            break;
        }
    }

    Ok(total_deleted)
}
