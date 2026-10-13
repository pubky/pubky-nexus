//! Keeps posts by authors outside the trust ranking out of the sorted sets every
//! viewer shares (global and per-tag). Every write goes through [`add`] or
//! [`incr`], which check the author in the same atomic step, and [`reconcile()`]
//! applies a new ranking, or a toggled `[features]` switch, to the posts already
//! there. The scripts assume a single Redis instance.

mod reconcile;
mod scripts;
#[cfg(test)]
mod tests;

pub(crate) use reconcile::{reconcile, AuthorPost};

use std::sync::atomic::{AtomicBool, Ordering};

use redis::AsyncCommands;

use crate::db::get_redis_conn;
use crate::db::kv::{sorted_key, RedisResult, ScoreAction};
use crate::models::user::SocialGraphStatus;
use scripts::{add_call, INCR};

/// A copy of the ranking the shared sets were last reconciled with. Absent
/// while nothing is filtered.
pub const APPLIED_KEY_PARTS: [&str; 2] = ["TrustFilter", "Applied"];
/// The most entries one script call writes or removes.
const BATCH: usize = 500;

/// Whether this process hides unranked authors, from `[features]
/// hide_unranked_authors`. On until [`FeaturesConfig::apply`] says otherwise,
/// so the API, which never sets it, follows the applied copy alone.
///
/// [`FeaturesConfig::apply`]: crate::FeaturesConfig::apply
static ENABLED: AtomicBool = AtomicBool::new(true);

/// Turns the filter on or off for this process.
pub(crate) fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

pub(crate) fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

fn applied_key() -> String {
    sorted_key(&APPLIED_KEY_PARTS)
}

/// Whether a ranking has been applied to the shared sets, which is when
/// Cypher-served streams hide the same authors.
pub(crate) async fn is_ranking_applied() -> RedisResult<bool> {
    if !is_enabled() {
        return Ok(false);
    }
    let mut conn = get_redis_conn().await?;
    Ok(conn.exists(applied_key()).await?)
}

/// Adds `entries` (`(score, "author:post")`) to the shared sorted set at
/// `key_parts`, leaving out those whose author the filter hides.
pub(crate) async fn add(key_parts: &[&str], entries: &[(f64, &str)]) -> RedisResult<()> {
    let key = sorted_key(key_parts);
    let mut conn = get_redis_conn().await?;
    for batch in entries.chunks(BATCH) {
        let call = add_call(&key, !is_enabled(), batch.iter().copied());
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
    let _: u8 = INCR
        .key(sorted_key(key_parts))
        .key(SocialGraphStatus::ranking_key())
        .arg(u8::from(!is_enabled()))
        .arg(delta)
        .arg(member.join(":"))
        .invoke_async(&mut conn)
        .await?;
    Ok(())
}
