//! Stubs: users this Nexus knows but keeps outside its scope.
//!
//! A stub's node exists so in-scope records (a follow, a tag, a reply) can point at it,
//! but its own content is never read: it has no `HOSTED_BY` and no cursor. It has
//! counts, which in-scope records move, but is kept out of the rankings and its posts
//! out of the global feeds, where a profile never read would show as a blank card.
//!
//! The graph's `scope_state` is the source of truth; `Scope:Stubs` mirrors it for the
//! per-write check and is rebuilt from the graph at start.

use crate::db::kv::{sets, RedisResult};
use crate::db::{exec_single_row, fetch_key_from_graph, queries, RedisOps};
use crate::models::error::ModelResult;
use crate::models::user::UserCounts;

/// `scope_state` of a user known to this Nexus but outside its scope.
pub const SCOPE_STATE_STUB: &str = "stub";
const STUBS: (&str, &str) = ("Scope", "Stubs");

pub async fn is_stub(user_id: &str) -> RedisResult<bool> {
    sets::is_member(STUBS.0, STUBS.1, user_id).await
}

/// Makes an existing user node a stub, with zeroed counts outside the rankings.
pub async fn mark_stub(user_id: &str) -> ModelResult<()> {
    let query = queries::put::set_user_scope_state(user_id, Some(SCOPE_STATE_STUB));
    exec_single_row(query).await?;
    sets::put(STUBS.0, STUBS.1, &[user_id], None).await?;
    UserCounts::default()
        .put_index_json(&[user_id], None, None)
        .await?;
    Ok(())
}

/// Stops treating the user as a stub in Redis and puts its counts into the rankings.
/// Returns whether it was a stub. The caller clears its `scope_state`.
pub async fn unstub(user_id: &str) -> ModelResult<bool> {
    if !sets::take(STUBS.0, STUBS.1, user_id).await? {
        return Ok(false);
    }
    if let Some(counts) = UserCounts::get_from_index(user_id).await? {
        counts.put_to_index(user_id).await?;
    }
    Ok(true)
}

/// Rebuilds `Scope:Stubs` from the graph. Returns how many stubs the graph holds.
pub async fn rebuild_stub_index() -> ModelResult<usize> {
    let query = queries::get::get_user_ids_by_scope_state(SCOPE_STATE_STUB);
    let stubs: Vec<String> = fetch_key_from_graph(query, "user_ids")
        .await?
        .unwrap_or_default();
    sets::replace(STUBS.0, STUBS.1, &stubs).await?;
    Ok(stubs.len())
}
