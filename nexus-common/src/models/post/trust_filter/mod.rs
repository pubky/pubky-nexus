//! Keeps posts by authors outside the trust ranking out of the sorted sets every
//! viewer shares (global, per-tag and per-thread). Every write goes through
//! [`add`] or [`incr`], which check the author in the same atomic step, and
//! [`reconcile()`] applies a new ranking, or a toggled `[features]` switch, to
//! the posts already there. The scripts assume a single Redis instance.

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
const APPLIED_KEY_PARTS: [&str; 2] = ["TrustFilter", "Applied"];
/// The most entries one script call writes or removes.
const BATCH: usize = 500;

/// Whether this process hides unranked authors, from `[features]
/// hide_unranked_authors`. On until [`FeaturesConfig::apply`] says otherwise.
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

/// Whether the filter lets `author` through: everyone with the filter off or
/// no ranking, otherwise only ranked authors.
pub(crate) async fn admits(author: &str) -> RedisResult<bool> {
    if !is_enabled() {
        return Ok(true);
    }
    let ranking = SocialGraphStatus::ranking_key();
    let mut conn = get_redis_conn().await?;
    let (has_ranking, rank): (bool, Option<f64>) = redis::pipe()
        .exists(&ranking)
        .zscore(&ranking, author)
        .query_async(&mut conn)
        .await?;
    Ok(!has_ranking || rank.is_some())
}

/// Adds `entries` (`(score, "author:post")`) to the shared sorted set at
/// `key_parts`, leaving out those whose author the filter hides.
pub(crate) async fn add(key_parts: &[&str], entries: &[(f64, &str)]) -> RedisResult<()> {
    add_batches(&sorted_key(key_parts), entries, !is_enabled()).await
}

/// Adds `entries` to the shared sorted set at `key_parts` whatever their
/// authors' rank.
pub(crate) async fn add_always(key_parts: &[&str], entries: &[(f64, &str)]) -> RedisResult<()> {
    add_batches(&sorted_key(key_parts), entries, true).await
}

async fn add_batches(key: &str, entries: &[(f64, &str)], always: bool) -> RedisResult<()> {
    let mut conn = get_redis_conn().await?;
    for batch in entries.chunks(BATCH) {
        let call = add_call(key, always, batch.iter().copied());
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
