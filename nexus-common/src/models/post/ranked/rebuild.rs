//! Rebuilding the ranked sets after a ranking publish: each one reconciled with
//! its source in place, one atomic script per page.

use std::collections::BTreeSet;

use deadpool_redis::Connection;
use redis::{AsyncCommands, ScanOptions};

use super::scripts::{PRUNE, RECONCILE};
use super::{
    ranking_key, sorted_key, RankedSet, BUILT_AT_KEY, TAG_GLOBAL_POST_TIMELINE,
    TAG_RANKED_POST_TIMELINE,
};
use crate::db::get_redis_conn;
use crate::db::kv::{RedisResult, SORTED_PREFIX};

/// Members per `ZSCAN` page, the most a rebuild call touches. Sized so a call
/// stays well under 10 ms of Redis time: at 200k root posts the slowest took 3 ms.
const BATCH: usize = 500;
/// Keys per `SCAN` page when listing labels; cheap per key, so larger.
const SCAN_COUNT: usize = 1_000;
/// Keys per `UNLINK` when dropping the ranked sets.
const UNLINK_BATCH: usize = 1_000;

/// Rebuilds every ranked set from the current ranking, or drops them all when
/// there is no ranking. Takes no lock: every page reads the live ranking, so
/// rebuilds that overlap converge on the same sets. In production only the
/// trust job rebuilds, one run at a time under its job lock.
pub(crate) async fn rebuild() -> RedisResult<RankedRebuildStats> {
    let mut conn = get_redis_conn().await?;
    let mut stats = RankedRebuildStats::default();
    let ranking_exists: bool = conn.exists(ranking_key()).await?;
    if !ranking_exists {
        drop_all(&mut conn).await?;
        stats.dropped = true;
        return Ok(stats);
    }

    let tags = scan_tag_keys(&mut conn).await?;
    // A ranked set normally empties, and so vanishes, with its source; one that
    // outlived its source drifted, and reconciling it against the missing
    // source empties it.
    stats.orphans = tags.ranked.difference(&tags.sources).count();
    let labels = tags
        .sources
        .union(&tags.ranked)
        .map(|label| RankedSet::tag(label));
    for set in std::iter::once(RankedSet::global()).chain(labels) {
        rebuild_set(&mut conn, &set, &mut stats).await?;
    }

    let built_at = chrono::Utc::now().timestamp_millis();
    let _: () = conn.set(BUILT_AT_KEY, built_at).await?;
    Ok(stats)
}

/// Drops every ranked set. The ready marker goes first, so readers fall back
/// to the unfiltered sets before any ranked set disappears.
async fn drop_all(conn: &mut Connection) -> RedisResult<()> {
    let _: () = conn.del(BUILT_AT_KEY).await?;
    let _: () = conn.unlink(RankedSet::global().ranked).await?;
    let tags = scan_tag_keys(conn).await?;
    let ranked = tags.ranked.iter().map(|label| RankedSet::tag(label).ranked);
    unlink_keys(conn, ranked).await
}

/// The labels of every per-label family, from one pass over the keyspace:
/// `SCAN` costs the whole keyspace however few keys match, so the families
/// share it. Collected up front: a rebuild only writes keys of families it
/// has already listed, and the sets absorb the repeats `SCAN` may return.
async fn scan_tag_keys(conn: &mut Connection) -> RedisResult<TagKeys> {
    let prefix = |parts: &[&str]| format!("{}:", sorted_key(parts));
    let source_prefix = prefix(&TAG_GLOBAL_POST_TIMELINE);
    let ranked_prefix = prefix(&TAG_RANKED_POST_TIMELINE);
    let [tags_part, _, post_part, timeline_part] = TAG_RANKED_POST_TIMELINE;
    let pattern = format!("{SORTED_PREFIX}:{tags_part}:*:{post_part}:{timeline_part}:*");

    let scan = ScanOptions::default()
        .with_pattern(pattern)
        .with_count(SCAN_COUNT);
    let mut keys = conn.scan_options::<String>(scan).await?;
    let mut tags = TagKeys::default();
    while let Some(key) = keys.next_item().await {
        let key = key?;
        if let Some(label) = key.strip_prefix(source_prefix.as_str()) {
            tags.sources.insert(label.to_string());
        } else if let Some(label) = key.strip_prefix(ranked_prefix.as_str()) {
            tags.ranked.insert(label.to_string());
        }
    }
    Ok(tags)
}

/// The labels found for each per-label family.
#[derive(Debug, Default)]
struct TagKeys {
    sources: BTreeSet<String>,
    ranked: BTreeSet<String>,
}

/// What a rebuild did, for its log line.
#[derive(Debug, Default)]
pub(crate) struct RankedRebuildStats {
    /// Ranked sets rebuilt (the global one plus one per label).
    pub sets: usize,
    /// Source members examined.
    pub scanned: usize,
    /// Members added to, or rescored in, a ranked set.
    pub added: usize,
    /// Members removed from a ranked set.
    pub removed: usize,
    /// Ranked sets whose source set no longer existed.
    pub orphans: usize,
    /// No ranking existed, so every ranked set was dropped instead.
    pub dropped: bool,
}

/// Reconciles `set.ranked` with `set.source` in place, a page at a time.
/// Concurrent writes stay correct: each page is one atomic script, and
/// [`add`](super::add) and [`remove`](super::remove) keep the copy in step with
/// the source in between.
pub(super) async fn rebuild_set(
    conn: &mut Connection,
    set: &RankedSet,
    stats: &mut RankedRebuildStats,
) -> RedisResult<()> {
    stats.sets += 1;
    let mut pruned = false;
    let mut cursor = 0;
    loop {
        let (next, scanned, added, removed, page_pruned) =
            reconcile_page(conn, set, cursor).await?;
        stats.scanned += scanned;
        stats.added += added;
        stats.removed += removed;
        pruned |= page_pruned;
        if next == 0 {
            break;
        }
        cursor = next;
    }
    // A ranked set too big to prune in the first call, a page at a time.
    let mut cursor = 0;
    while !pruned {
        let (next, removed) = prune_page(conn, set, cursor).await?;
        stats.removed += removed;
        pruned = next == 0;
        cursor = next;
    }
    Ok(())
}

/// `(next cursor, scanned, added, removed, pruned)` for the source page at `cursor`.
pub(super) async fn reconcile_page(
    conn: &mut Connection,
    set: &RankedSet,
    cursor: u64,
) -> RedisResult<(u64, usize, usize, usize, bool)> {
    RECONCILE
        .key(&set.source)
        .key(ranking_key())
        .key(&set.ranked)
        .arg(cursor)
        .arg(BATCH)
        .invoke_async(conn)
        .await
        .map_err(Into::into)
}

/// `(next cursor, removed)` for the ranked page at `cursor`.
async fn prune_page(
    conn: &mut Connection,
    set: &RankedSet,
    cursor: u64,
) -> RedisResult<(u64, usize)> {
    PRUNE
        .key(&set.ranked)
        .key(&set.source)
        .arg(cursor)
        .arg(BATCH)
        .invoke_async(conn)
        .await
        .map_err(Into::into)
}

pub(super) async fn unlink_keys(
    conn: &mut Connection,
    keys: impl IntoIterator<Item = String>,
) -> RedisResult<()> {
    let keys: Vec<String> = keys.into_iter().collect();
    for chunk in keys.chunks(UNLINK_BATCH) {
        let _: () = conn.unlink(chunk).await?;
    }
    Ok(())
}
