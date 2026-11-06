//! Keeps posts by authors outside the trust ranking out of the sorted sets every
//! viewer shares (global and per-tag). Every write goes through [`add`] or
//! [`incr`], which check the author in the same atomic step, and [`reconcile()`]
//! applies a new ranking, or a toggled `[features]` switch, to the posts already
//! there. The switch lives in Redis, so every service follows the one the
//! watcher stored. The scripts assume a single Redis instance.

mod reconcile;
mod scripts;
#[cfg(test)]
mod tests;

pub(crate) use reconcile::{author_entries, reconcile, show_entries, withdraw_entries, AuthorPost};

use redis::AsyncCommands;

use crate::db::get_redis_conn;
use crate::db::kv::{sorted_key, RedisResult, ScoreAction};
use scripts::{add_call, gate_call, INCR};

/// A copy of the ranking the shared sets were last reconciled with. Absent
/// while nothing is filtered.
pub const APPLIED_KEY_PARTS: [&str; 2] = ["TrustFilter", "Applied"];
/// Set before switching on hides anything, and cleared once switching off has
/// shown every author again: while it exists the shared sets may hide posts,
/// even with no applied copy.
const FILTERED_KEY: &str = "TrustFilter:Filtered";
/// Set while switching off writes authors back, until it finishes. A switch-on
/// that finds it can't diff against the copy: authors outside the ranking may
/// already be back.
const SHOWING_KEY: &str = "TrustFilter:Showing";
/// Present while the filter is on: the switch every service follows, which the
/// watcher stores from `[features] hide_unranked_authors` at startup.
const ON_KEY: &str = "TrustFilter:On";
/// The most entries one script call writes or removes.
const BATCH: usize = 500;

/// Stores the switch. Switching on records that posts may be hidden in the
/// same step, before the gate hides anything. Switching off opens the gate at
/// once, so nothing written while [`reconcile()`] shows the hidden authors
/// again is held back.
pub(crate) async fn switch(on: bool) -> RedisResult<()> {
    let mut conn = get_redis_conn().await?;
    let _: () = match on {
        true => conn.mset(&[(FILTERED_KEY, 1), (ON_KEY, 1)]).await?,
        false => conn.del(ON_KEY).await?,
    };
    Ok(())
}

/// Whether the shared sets may hide posts: the filter is on, or switching it
/// off hasn't finished.
pub(crate) async fn may_hide_posts() -> RedisResult<bool> {
    let mut conn = get_redis_conn().await?;
    Ok(conn.exists(FILTERED_KEY).await?)
}

fn applied_key() -> String {
    sorted_key(&APPLIED_KEY_PARTS)
}

/// Whether the filter is on and a ranking has been applied to the shared sets,
/// which is when Cypher-served streams hide the same authors.
pub(crate) async fn is_ranking_applied() -> RedisResult<bool> {
    let mut conn = get_redis_conn().await?;
    let present: usize = conn.exists(&[ON_KEY.to_string(), applied_key()]).await?;
    Ok(present == 2)
}

/// Adds `entries` (`(score, "author:post")`) to the shared sorted set at
/// `key_parts`, leaving out those whose author the filter hides.
pub(crate) async fn add(key_parts: &[&str], entries: &[(f64, &str)]) -> RedisResult<()> {
    let key = sorted_key(key_parts);
    let mut conn = get_redis_conn().await?;
    for batch in entries.chunks(BATCH) {
        let call = add_call(&key, batch.iter().copied());
        let _: usize = call.invoke_async(&mut conn).await?;
    }
    Ok(())
}

/// Moves the score of `member` (`[author, post]`) in the shared sorted set at
/// `key_parts` by `action`. A member that isn't there yet is created only by
/// an increment, and only when the filter lets its author in.
pub(crate) async fn incr(
    key_parts: &[&str],
    member: &[&str],
    action: ScoreAction,
) -> RedisResult<()> {
    let delta = match action {
        ScoreAction::Increment(value) => value,
        ScoreAction::Decrement(value) => -value,
    };
    let mut conn = get_redis_conn().await?;
    let _: u8 = gate_call(&INCR, &sorted_key(key_parts))
        .arg(delta)
        .arg(member.join(":"))
        .invoke_async(&mut conn)
        .await?;
    Ok(())
}
