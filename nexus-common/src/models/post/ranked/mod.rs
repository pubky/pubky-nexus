//! Ranked copies of the sorted sets behind `source=all` with `sorting=timeline`:
//! the same members at the same scores, keeping only posts whose author is in
//! the trust ranking (`Sorted:Users:SocialGraph`, built by the trust job).
//!
//! One ranked set mirrors the global timeline and one mirrors each per-label
//! timeline. Serving a filtered stream from a pre-filtered set keeps paging
//! exact: a page is only short at the real end of the stream, which matters
//! because pubky-app treats a short page as the end of the feed.
//!
//! Every incremental write to a source set goes through [`add`] and [`remove`],
//! which update the ranked copy in the same atomic step; the bulk reindex
//! writes the sources directly and is followed by a rebuild. Every ranking publish
//! reconciles the copies with their sources in place, a page at a time
//! ([`rebuild()`]), so drift lasts at most until the next recompute and no call
//! holds Redis for more than one page. Nothing is filtered until a complete
//! rebuild has run ([`BUILT_AT_KEY`]): before that every shape, Cypher included,
//! serves the unfiltered stream, so the filter switches on everywhere at once.
//!
//! The scripts take several keys, so they assume a single Redis instance (not
//! Redis Cluster), as the rest of Nexus does.

mod rebuild;
mod scripts;
#[cfg(test)]
mod tests;

pub(crate) use rebuild::rebuild;

use super::search::TAG_GLOBAL_POST_TIMELINE;
use super::stream::POST_TIMELINE_KEY_PARTS;
use crate::db::get_redis_conn;
use crate::db::kv::{RedisResult, SORTED_PREFIX};
use crate::models::user::USER_SOCIAL_GRAPH_KEY_PARTS;
use scripts::ADD;

/// Ranked copy of the global timeline: `Sorted:Posts:Ranked:Timeline`.
/// A different second segment from the source set, so a glob for one never matches the other.
pub const POST_RANKED_TIMELINE_KEY_PARTS: [&str; 3] = ["Posts", "Ranked", "Timeline"];
/// Ranked copies of the per-label timelines: `Sorted:Tags:Ranked:Post:Timeline:<label>`.
/// A different second segment from the source sets, so a scan for one never matches the other.
pub const TAG_RANKED_POST_TIMELINE: [&str; 4] = ["Tags", "Ranked", "Post", "Timeline"];
/// Set once a complete rebuild has run; nothing is filtered before. Deleting it
/// turns the filter off on every shape until the next rebuild.
const BUILT_AT_KEY: &str = "Ranked:Timeline:BuiltAt";

/// The key of the sorted set at `parts`.
fn sorted_key(parts: &[&str]) -> String {
    format!("{SORTED_PREFIX}:{}", parts.join(":"))
}

/// The trust ranking: users with a positive trust score whose profile isn't deleted.
fn ranking_key() -> String {
    sorted_key(&USER_SOCIAL_GRAPH_KEY_PARTS)
}

/// A source set and its ranked copy.
#[derive(Debug)]
pub(crate) struct RankedSet {
    pub source: String,
    pub ranked: String,
}

impl RankedSet {
    /// The global timeline.
    pub fn global() -> Self {
        RankedSet {
            source: sorted_key(&POST_TIMELINE_KEY_PARTS),
            ranked: sorted_key(&POST_RANKED_TIMELINE_KEY_PARTS),
        }
    }

    /// The per-label timeline for `label`.
    pub fn tag(label: &str) -> Self {
        let key = |parts: &[&str]| sorted_key(&[parts, &[label]].concat());
        RankedSet {
            source: key(&TAG_GLOBAL_POST_TIMELINE),
            ranked: key(&TAG_RANKED_POST_TIMELINE),
        }
    }
}

/// How the trust ranking applies to one in-scope request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrustMode {
    /// Served exactly as without the filter: no ranking, a viewer outside it,
    /// or no complete rebuild yet.
    Off,
    /// Filtered: Redis shapes read the ranked sets, Cypher shapes add the rule.
    Ranked,
}

impl TrustMode {
    /// Reads the ranking state for one request in one round trip.
    pub async fn load(viewer_id: Option<&str>) -> RedisResult<Self> {
        let ranking = ranking_key();
        let mut conn = get_redis_conn().await?;
        let mut pipe = redis::pipe();
        pipe.exists(BUILT_AT_KEY);
        // A viewer's rank also says that a ranking exists.
        let (built, filtered) = match viewer_id {
            Some(viewer_id) => {
                let (built, rank): (bool, Option<f64>) = pipe
                    .zscore(&ranking, viewer_id)
                    .query_async(&mut conn)
                    .await?;
                (built, rank.is_some())
            }
            None => pipe.exists(&ranking).query_async(&mut conn).await?,
        };
        Ok(Self::decide(filtered, built))
    }

    /// `filtered` says whether the filter applies to the request at all: a
    /// ranking exists and the viewer, if any, is in it. A viewer outside the
    /// ranking, including one Nexus does not know, gets the unfiltered stream.
    /// `built` says a complete rebuild has run. Until then no shape filters, so
    /// a deploy onto an instance that already publishes a ranking (for the
    /// badge) switches the filter on at its first rebuild, on every shape at once.
    pub fn decide(filtered: bool, built: bool) -> Self {
        if filtered && built {
            TrustMode::Ranked
        } else {
            TrustMode::Off
        }
    }
}

/// Adds `member` (`author:post`) to `set.source` at `score`, and to the ranked
/// copy when its author is in the ranking.
pub(crate) async fn add(set: &RankedSet, member: &str, score: f64) -> RedisResult<()> {
    let mut conn = get_redis_conn().await?;
    let _: () = ADD
        .key(&set.source)
        .key(ranking_key())
        .key(&set.ranked)
        .arg(member)
        .arg(score)
        .invoke_async(&mut conn)
        .await?;
    Ok(())
}

/// Removes `member` from `set.source` and its ranked copy in one transaction.
pub(crate) async fn remove(set: &RankedSet, member: &str) -> RedisResult<()> {
    let mut conn = get_redis_conn().await?;
    let _: () = redis::pipe()
        .atomic()
        .zrem(&set.source, member)
        .ignore()
        .zrem(&set.ranked, member)
        .ignore()
        .query_async(&mut conn)
        .await?;
    Ok(())
}
