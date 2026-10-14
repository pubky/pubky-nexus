//! Stubs: users this Nexus knows but keeps outside its scope.
//!
//! A stub's node exists so in-scope records (a follow, a tag, a reply) can point at
//! it, but its own content is never read. It has no `HOSTED_BY` edge and no cursor,
//! so the HS resolver and the indexers never touch it, and it is kept out of the
//! global rankings and feeds, where a user whose profile was never read would show
//! up as a blank card.
//!
//! The graph's `scope_state` is the source of truth. A Redis set mirrors it so the
//! checks on every count and feed write are one lookup; it is rebuilt from the graph
//! when the watcher starts.

use crate::db::get_redis_conn;
use crate::db::kv::{RedisError, RedisResult};
use crate::db::{exec_single_row, fetch_key_from_graph, queries};
use crate::models::error::ModelResult;

/// `scope_state` of a user known to this Nexus but outside its scope.
pub const SCOPE_STATE_STUB: &str = "stub";
/// Redis set of the stubs' ids, mirroring their `scope_state` in the graph.
const STUBS_KEY: &str = "Scope:Stubs";

/// Whether the user is a stub. One Redis lookup.
pub async fn is_stub(user_id: &str) -> RedisResult<bool> {
    let mut redis_conn = get_redis_conn().await?;
    let is_member: bool = redis::cmd("SISMEMBER")
        .arg(STUBS_KEY)
        .arg(user_id)
        .query_async(&mut redis_conn)
        .await?;
    Ok(is_member)
}

/// Marks an existing user node as a stub, in the graph and in Redis.
pub async fn mark_stub(user_id: &str) -> ModelResult<()> {
    let query = queries::put::set_user_scope_state(user_id, Some(SCOPE_STATE_STUB));
    exec_single_row(query).await?;
    let mut redis_conn = get_redis_conn().await?;
    let () = redis::cmd("SADD")
        .arg(STUBS_KEY)
        .arg(user_id)
        .query_async(&mut redis_conn)
        .await
        .map_err(RedisError::from)?;
    Ok(())
}

/// Removes the user's stub mark from Redis. The caller sets its new `scope_state`.
pub async fn unmark_stub(user_id: &str) -> RedisResult<()> {
    let mut redis_conn = get_redis_conn().await?;
    let () = redis::cmd("SREM")
        .arg(STUBS_KEY)
        .arg(user_id)
        .query_async(&mut redis_conn)
        .await?;
    Ok(())
}

/// Rebuilds the Redis set from the graph, so a Redis wipe or restore loses no stub.
/// Returns how many stubs the graph holds.
pub async fn rebuild_stub_index() -> ModelResult<usize> {
    let query = queries::get::get_user_ids_by_scope_state(SCOPE_STATE_STUB);
    let stubs: Vec<String> = fetch_key_from_graph(query, "user_ids")
        .await?
        .unwrap_or_default();

    let mut pipe = redis::pipe();
    pipe.atomic().del(STUBS_KEY).ignore();
    for chunk in stubs.chunks(1_000) {
        pipe.sadd(STUBS_KEY, chunk).ignore();
    }
    let mut redis_conn = get_redis_conn().await?;
    let () = pipe
        .query_async(&mut redis_conn)
        .await
        .map_err(RedisError::from)?;
    Ok(stubs.len())
}
