//! Users brought into the scope whose earlier records were skipped.
//!
//! A user outside the scope is not read, so the events it wrote meanwhile were
//! skipped, also in the feeds of the homeservers read whole. Once it is let in, its
//! own stream is read from the start until it is empty, on any homeserver; on a
//! homeserver read whole, only for these users. Records read twice are indexed once.

use std::collections::HashSet;

use deadpool_redis::redis::AsyncCommands;

use crate::db::get_redis_conn;
use crate::db::kv::{RedisError, RedisResult};

/// Users whose own stream is read until it is empty: user id -> homeserver id.
const CATCH_UP_KEY: &str = "Scope:CatchUp";

/// Has the user's own stream on `hs_id` read from the start.
pub async fn start_catch_up(user_id: &str, hs_id: &str) -> RedisResult<()> {
    let mut conn = get_redis_conn().await?;
    let () = conn
        .hset(CATCH_UP_KEY, user_id, hs_id)
        .await
        .map_err(RedisError::from)?;
    Ok(())
}

/// The homeservers of the users catching up.
pub async fn homeservers_catching_up() -> RedisResult<HashSet<String>> {
    let mut conn = get_redis_conn().await?;
    let hs_ids: Vec<String> = conn.hvals(CATCH_UP_KEY).await.map_err(RedisError::from)?;
    Ok(hs_ids.into_iter().collect())
}

/// Which of `user_ids` are catching up, in order.
pub async fn catching_up(user_ids: &[&str]) -> RedisResult<Vec<bool>> {
    if user_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut conn = get_redis_conn().await?;
    let hs_ids: Vec<Option<String>> = conn
        .hmget(CATCH_UP_KEY, user_ids)
        .await
        .map_err(RedisError::from)?;
    Ok(hs_ids.iter().map(Option::is_some).collect())
}

/// The user's stream has been read to its end.
pub async fn caught_up(user_id: &str) -> RedisResult<()> {
    let mut conn = get_redis_conn().await?;
    let () = conn
        .hdel(CATCH_UP_KEY, user_id)
        .await
        .map_err(RedisError::from)?;
    Ok(())
}
