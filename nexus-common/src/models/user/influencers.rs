use crate::db::graph::keyset_scan;
use crate::db::kv::RedisResult;
use crate::db::kv::SortOrder;
use crate::models::error::{ModelError, ModelResult};
use crate::types::{CacheTimeframe, StreamReach, Timeframe};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use std::sync::{Mutex, PoisonError};
use tokio::sync::Mutex as AsyncMutex;
use tracing::{debug, warn};
use utoipa::ToSchema;

use super::{UserDetails, USER_INFLUENCERS_KEY_PARTS};
use crate::db::{fetch_all_rows_from_graph, fetch_key_from_graph, queries, RedisOps};

const GLOBAL_INFLUENCERS_PREFIX: &str = "Cache:Influencers";
/// Users per graph read when the AllTime ranking is rebuilt.
const ALL_TIME_PAGE_SIZE: usize = 500;
/// How many global influencers each cache-backed timeframe holds. Pages past this
/// are empty by construction, so callers should reject skips beyond it rather than
/// treat the empty page as a cache miss.
pub const GLOBAL_INFLUENCERS_CACHE_SIZE: usize = 100;

/// How many influencers a `preview` response returns.
pub const GLOBAL_INFLUENCERS_PREVIEW_LIMIT: usize = 3;

/// How many offsets a `preview` response can start at: the cached ranking minus the
/// entries a full window needs, so every offset still yields a complete window.
pub const GLOBAL_INFLUENCERS_PREVIEW_OFFSETS: u32 =
    (GLOBAL_INFLUENCERS_CACHE_SIZE - GLOBAL_INFLUENCERS_PREVIEW_LIMIT + 1) as u32;

/// Offset of a `preview` window for a clock sample; production passes
/// `Utc::now().timestamp_subsec_micros()`.
///
/// Pure so the mapping is testable without the clock. Two samples collide whenever they
/// differ by a multiple of `GLOBAL_INFLUENCERS_PREVIEW_OFFSETS` microseconds (98), e.g.
/// samples 4998 us apart land on the same offset. Two consecutive preview requests are
/// therefore NOT guaranteed to return different influencers.
pub fn preview_skip(micros: u32) -> usize {
    (micros % GLOBAL_INFLUENCERS_PREVIEW_OFFSETS) as usize
}

/// The lock a request takes when its ranking's key is missing, one per timeframe:
/// requests that miss the same ranking together share one rebuild, and no ranking
/// waits for another's.
fn rebuild_lock(timeframe: &Timeframe) -> &'static AsyncMutex<()> {
    static TODAY: AsyncMutex<()> = AsyncMutex::const_new(());
    static THIS_WEEK: AsyncMutex<()> = AsyncMutex::const_new(());
    static THIS_MONTH: AsyncMutex<()> = AsyncMutex::const_new(());
    static ALL_TIME: AsyncMutex<()> = AsyncMutex::const_new(());
    match timeframe {
        Timeframe::Today => &TODAY,
        Timeframe::ThisWeek => &THIS_WEEK,
        Timeframe::ThisMonth => &THIS_MONTH,
        Timeframe::AllTime => &ALL_TIME,
    }
}

#[derive(Serialize, Deserialize, Debug, ToSchema, Default, Clone)]
pub struct Influencers(pub Vec<(String, f64)>); // (user_id, score)

impl RedisOps for Influencers {}

// Create a Influencers instance directly from an iterator of Influencer items
// Need it in collect()
impl FromIterator<(String, f64)> for Influencers {
    fn from_iter<I: IntoIterator<Item = (String, f64)>>(iter: I) -> Self {
        Influencers(iter.into_iter().collect())
    }
}

// Implement Deref so Influencers can be used like Vec<String>
impl Deref for Influencers {
    type Target = Vec<(String, f64)>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Influencers {
    /// Retrieves a list of influencers based on the provided context.
    ///
    /// If a `user_id` is provided, the function returns influencers relevant to the user
    /// using the specified `reach` level (e.g., friends, followers). Otherwise, it returns
    /// global influencers. When `preview` mode is enabled, it overrides the `skip` and `limit`
    /// values with pseudo-random values to support randomized previews
    ///
    /// # Arguments
    ///
    /// * `user_id` - Optional user ID to fetch influencers relative to the user
    /// * `reach` - Optional reach filter (e.g., Friends, Followers) for user-scoped queries
    /// * `skip` - Number of results to skip (ignored in preview mode)
    /// * `limit` - Maximum number of results to return (ignored in preview mode)
    /// * `timeframe` - Time range to filter influencer activity
    /// * `preview` - If true, uses pseudo-random pagination to return a small randomized subset
    ///
    pub async fn get_influencers(
        user_id: Option<&str>,
        reach: Option<StreamReach>,
        skip: usize,
        limit: usize,
        timeframe: Timeframe,
        preview: bool,
    ) -> ModelResult<Option<Influencers>> {
        let (skip, limit) = if preview {
            // The offset is derived from the clock, so two requests made within the same
            // microsecond, or a multiple of `GLOBAL_INFLUENCERS_PREVIEW_OFFSETS`
            // microseconds apart, return the same window.
            let skip = preview_skip(Utc::now().timestamp_subsec_micros());
            debug!("Influencer preview active: skip number {}", skip);
            (skip, GLOBAL_INFLUENCERS_PREVIEW_LIMIT)
        } else {
            (skip, limit)
        };
        if let Some(user) = user_id.filter(|_| timeframe != Timeframe::AllTime) {
            return Influencers::get_influencers_by_reach(
                user,
                reach.unwrap_or(StreamReach::Friends),
                skip,
                limit,
                &timeframe,
            )
            .await;
        }
        Influencers::get_global_influencers(skip, limit, &timeframe).await
    }

    /// It first attempts to fetch a subset of global influencers from cache
    /// based on the provided `skip` and `limit`. Only a missing cache key counts as a
    /// miss: then it rebuilds the ranking from the graph with [`Self::refresh`] and
    /// retrieves the requested subset again from cache. One rebuild per ranking runs at
    /// a time: a request that waited for another's checks the key again first.
    /// An existing key whose window is empty (skip past the end, or every user in
    /// the window deleted) is served as an empty page, since refetching would rebuild
    /// the same ranking and rewrite the key on every such request.
    ///
    /// # Arguments
    ///
    /// * `skip` - Number of entries to skip (for pagination)
    /// * `limit` - Maximum number of influencers to return
    /// * `timeframe` - The time range to filter influencer activity
    ///
    async fn get_global_influencers(
        skip: usize,
        limit: usize,
        timeframe: &Timeframe,
    ) -> ModelResult<Option<Influencers>> {
        let cached_influencers = Influencers::get_from_global_cache(skip, limit, timeframe).await?;
        if cached_influencers.is_some() {
            return Ok(cached_influencers);
        }

        let _rebuild = rebuild_lock(timeframe).lock().await;
        let cached_influencers = Influencers::get_from_global_cache(skip, limit, timeframe).await?;
        if cached_influencers.is_some() {
            return Ok(cached_influencers);
        }
        Influencers::refresh(timeframe).await?;
        Influencers::get_from_global_cache(skip, limit, timeframe)
            .await
            .map_err(Into::into)
    }

    /// Rebuilds the global ranking of `timeframe` from the graph: the top-100 cache of a
    /// ranged timeframe, or the whole AllTime ranking. The influencer jobs run it on their
    /// schedule, and a missing key falls back to it.
    pub async fn refresh(timeframe: &Timeframe) -> ModelResult<()> {
        match CacheTimeframe::from_timeframe(timeframe) {
            Some(cache_timeframe) => Influencers::fetch_and_cache(cache_timeframe).await,
            None => Influencers::refresh_all_time().await,
        }
    }

    /// Fetch top-100 influencers from the graph and atomically replace the cache.
    ///
    /// A transient empty or `None` graph result must not evict a good ranking, so the
    /// previous cache is preferred. The caller can retry on the next scheduled tick.
    async fn fetch_and_cache(timeframe: CacheTimeframe) -> ModelResult<()> {
        let query =
            queries::get::get_global_influencers(0, GLOBAL_INFLUENCERS_CACHE_SIZE, timeframe);
        let result = fetch_key_from_graph::<Influencers>(query, "influencers").await?;
        Influencers::write_or_preserve_cache(result, timeframe, GLOBAL_INFLUENCERS_PREFIX).await
    }

    /// Rebuilds the live `Sorted:Users:Influencers` set from the graph, so its members and
    /// scores converge on the graph's. Nothing else writes it and it has no TTL, so a
    /// stale ranking only changes here.
    async fn refresh_all_time() -> ModelResult<()> {
        let ranking = Influencers::read_all_time_ranking(ALL_TIME_PAGE_SIZE).await;
        Influencers::write_all_time_ranking(ranking, None).await
    }

    /// Scores every ranked user, `page_size` users per graph read.
    async fn read_all_time_ranking(page_size: usize) -> ModelResult<Vec<(String, f64)>> {
        let ranking = Mutex::new(Vec::new());
        keyset_scan(page_size, "influencers_all_time", |after_id| {
            let ranking = &ranking;
            async move {
                let query = queries::get::get_all_time_influencers(&after_id, page_size);
                let mut page: Vec<(String, f64)> = Vec::with_capacity(page_size);
                for row in fetch_all_rows_from_graph(query).await? {
                    page.push((row.get("id")?, row.get("score")?));
                }
                let last_id = page.last().map(|(id, _)| id.clone());
                let count = page.len();
                ranking
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .extend(page);
                Ok::<_, ModelError>((count, last_id))
            }
        })
        .await?;
        Ok(ranking.into_inner().unwrap_or_else(PoisonError::into_inner))
    }

    /// Swaps a complete ranking in with one atomic write. A failed read writes nothing,
    /// so the previous ranking is served until a later refresh succeeds. An empty one
    /// deletes the key: with no TTL, keeping the old ranking would keep its members
    /// forever. Tests drive it directly with injected results, under their own `prefix`.
    async fn write_all_time_ranking(
        ranking: ModelResult<Vec<(String, f64)>>,
        prefix: Option<&str>,
    ) -> ModelResult<()> {
        let ranking = ranking?;
        debug!(
            count = ranking.len(),
            "Replacing the AllTime influencers ranking"
        );
        let elements: Vec<(f64, &str)> = ranking
            .iter()
            .map(|(id, score)| (*score, id.as_str()))
            .collect();
        Influencers::replace_index_sorted_set(&USER_INFLUENCERS_KEY_PARTS, &elements, prefix, None)
            .await?;
        Ok(())
    }

    /// Writes a non-empty graph result to the cache, or preserves the existing cache
    /// when the result is empty or missing. This is the production half of the
    /// `fetch_and_cache` seam; tests drive it directly with injected results, under
    /// their own `prefix` so they never touch the keys the API serves from.
    async fn write_or_preserve_cache(
        result: Option<Influencers>,
        timeframe: CacheTimeframe,
        prefix: &str,
    ) -> ModelResult<()> {
        match result {
            Some(influencers) if !influencers.is_empty() => {
                debug!(
                    %timeframe,
                    count = influencers.len(),
                    "Writing influencer cache"
                );
                Influencers::put_to_global_cache(influencers, timeframe, prefix).await?;
            }
            Some(empty) => {
                warn!(
                    %timeframe,
                    count = empty.len(),
                    "Graph returned empty influencer set — previous cache left untouched"
                );
            }
            None => {
                warn!(
                    %timeframe,
                    "Graph returned no influencers — previous cache left untouched"
                );
            }
        }
        Ok(())
    }

    /// Retrieves a paginated list of global influencers from the cache for the given timeframe,
    /// filtering out deleted users and removing them from the cache.
    ///
    /// # Arguments
    ///
    /// * `skip` - Number of entries to skip in the sorted set
    /// * `limit` - Maximum number of influencers to return
    /// * `timeframe` - The time window to filter influencer rankings, used to generate the cache key
    async fn get_from_global_cache(
        skip: usize,
        limit: usize,
        timeframe: &Timeframe,
    ) -> RedisResult<Option<Influencers>> {
        let ranking = match timeframe {
            // AllTime reads the full ranking in Sorted:Users:Influencers. It has no TTL: the
            // all-time job rebuilds it, so a request only pays for a rebuild when the key is lost.
            Timeframe::AllTime => {
                Influencers::try_from_index_sorted_set(
                    super::USER_INFLUENCERS_KEY_PARTS.as_slice(),
                    None,
                    None,
                    Some(skip),
                    Some(limit),
                    SortOrder::Descending,
                    None,
                )
                .await?
            }

            // For all other timeframes, we fallback to the cache with TTL (Cache::Influencers::Timeframe)
            _ => {
                let cache_timeframe = CacheTimeframe::from_timeframe(timeframe)
                    .expect("every non-AllTime timeframe is cache-backed");
                let key_parts = Influencers::get_cache_key_parts(cache_timeframe);
                let key_parts_vector: Vec<&str> = key_parts.iter().map(|s| s.as_str()).collect();

                Influencers::try_from_index_sorted_set(
                    key_parts_vector.as_slice(),
                    None,
                    None,
                    Some(skip),
                    Some(limit),
                    SortOrder::Descending,
                    Some(GLOBAL_INFLUENCERS_PREFIX),
                )
                .await?
            }
        };

        // `None` means the key is absent. An empty window on an existing key stays
        // `Some`, so the caller does not mistake it for a miss and refetch.
        match ranking {
            Some(r) => Ok(Some(
                Influencers::filter_deleted(Influencers(r), Some(timeframe)).await?,
            )),
            None => Ok(None),
        }
    }

    /// Stores a list of global influencers as a sorted set for the given timeframe.
    ///
    /// The target key must match the one `get_from_global_cache` reads for that timeframe:
    /// `AllTime` reads the live `Sorted:Users:Influencers` set, every other timeframe reads
    /// `Cache:Influencers:{timeframe}`.
    ///
    /// # Arguments
    /// * `result` - The list of influencers with their scores to cache
    /// * `timeframe` - The timeframe used to generate the cache key and expiry
    /// * `prefix` - Key prefix; production passes `GLOBAL_INFLUENCERS_PREFIX`
    async fn put_to_global_cache(
        result: Influencers,
        timeframe: CacheTimeframe,
        prefix: &str,
    ) -> ModelResult<()> {
        let key_parts = Influencers::get_cache_key_parts(timeframe);
        let key_parts_vector: Vec<&str> =
            key_parts.iter().map(|s| s.as_str()).collect::<Vec<&str>>();

        // store the ranking as sorted set in cache
        Influencers::replace_index_sorted_set(
            key_parts_vector.as_slice(),
            result
                .iter()
                .map(|influencer| (influencer.1, influencer.0.as_str()))
                .collect::<Vec<(f64, &str)>>()
                .as_slice(),
            Some(prefix),
            Some(timeframe.to_cache_period()),
        )
        .await?;
        Ok(())
    }

    /// Retrieves influencers for a user based on the given `reach` and `timeframe` from the graph
    ///
    /// # Arguments
    /// * `user_id` - The ID of the user to scope the influencer query
    /// * `reach` - The reach filter (e.g., Friends, Followers)
    /// * `skip` - Number of results to skip (for pagination)
    /// * `limit` - Maximum number of influencers to return
    /// * `timeframe` - Time window to filter influencer activity
    async fn get_influencers_by_reach(
        user_id: &str,
        reach: StreamReach,
        skip: usize,
        limit: usize,
        timeframe: &Timeframe,
    ) -> ModelResult<Option<Influencers>> {
        let query = queries::get::get_influencers_by_reach(user_id, reach, skip, limit, timeframe);
        fetch_key_from_graph::<Influencers>(query, "influencers")
            .await
            .map_err(Into::into)
    }

    /// Filters out deleted users from an `Influencers` list.
    /// Optionally cleans deleted entries from the global cache for the given timeframe.
    async fn filter_deleted(
        influencers: Influencers,
        timeframe: Option<&Timeframe>,
    ) -> RedisResult<Influencers> {
        let ids: Vec<String> = influencers.iter().map(|(id, _)| id.clone()).collect();
        let details_list = UserDetails::mget(&ids).await?;

        let mut kept = Vec::new();
        let mut deleted_ids = Vec::new();

        for ((id, score), details) in influencers.0.into_iter().zip(details_list) {
            match details {
                Some(ref d) if !d.deleted => kept.push((id, score)),
                _ => deleted_ids.push(id),
            }
        }

        if let Some(tf) = timeframe {
            Influencers::remove_deleted_from_global_cache(&deleted_ids, tf).await;
        }

        Ok(Influencers(kept))
    }

    /// Removes deleted user IDs from the global influencer sorted sets in Redis.
    async fn remove_deleted_from_global_cache(deleted_ids: &[String], timeframe: &Timeframe) {
        if deleted_ids.is_empty() {
            return;
        }
        let refs: Vec<&str> = deleted_ids.iter().map(|s| s.as_str()).collect();

        match timeframe {
            Timeframe::AllTime => {
                let _ = Influencers::remove_from_index_sorted_set(
                    None,
                    USER_INFLUENCERS_KEY_PARTS.as_slice(),
                    &refs,
                )
                .await;
            }
            _ => {
                let cache_timeframe = CacheTimeframe::from_timeframe(timeframe)
                    .expect("every non-AllTime timeframe is cache-backed");
                let key_parts = Influencers::get_cache_key_parts(cache_timeframe);
                let key_parts_refs: Vec<&str> = key_parts.iter().map(|s| s.as_str()).collect();
                let _ = Influencers::remove_from_index_sorted_set(
                    Some(GLOBAL_INFLUENCERS_PREFIX),
                    &key_parts_refs,
                    &refs,
                )
                .await;
            }
        }
    }

    fn get_cache_key_parts(timeframe: CacheTimeframe) -> Vec<String> {
        vec![timeframe.to_string()]
    }

    /// Rebuilds the AllTime ranking and the `ThisMonth` cache from the graph.
    pub async fn reindex() -> ModelResult<()> {
        Influencers::refresh_all_time().await?;
        Influencers::fetch_and_cache(CacheTimeframe::ThisMonth).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::get_redis_conn;
    use crate::models::{traits::Collection, user::UserCounts};
    use crate::{types::DynError, StackConfig, StackManager};
    use deadpool_redis::redis;
    use pubky::Keypair;
    use pubky_social_specs::PubkyId;
    use std::collections::HashMap;
    use std::time::Duration;

    /// Keeps the cache tests off `GLOBAL_INFLUENCERS_PREFIX`: the API tests run against
    /// the same Redis and assert exact rankings under the production keys.
    const TEST_PREFIX: &str = "InfluencersCacheTest";

    /// The preview offset comes from the clock, so the mapping is the only thing a test
    /// may assert: never that two consecutive preview requests return different users.
    #[test]
    fn preview_skip_keeps_a_full_window_inside_the_cached_ranking() {
        assert_eq!(preview_skip(0), 0);
        assert_eq!(preview_skip(97), 97);
        // The offset count leaves room for `GLOBAL_INFLUENCERS_PREVIEW_LIMIT` entries.
        assert_eq!(preview_skip(98), 0);

        for micros in [0u32, 1, 97, 98, 999_999] {
            let skip = preview_skip(micros);
            assert!(skip + GLOBAL_INFLUENCERS_PREVIEW_LIMIT <= GLOBAL_INFLUENCERS_CACHE_SIZE);
        }

        // Clock samples a multiple of 98 us apart share an offset. This is the collision
        // that made `test_global_influencers_preview` fail intermittently (~1% of runs,
        // a 5 ms sleep plus request overhead can land exactly 4998 us after the first
        // sample): the two responses were then the same window.
        assert_eq!(preview_skip(1_000_000), preview_skip(1_000_000 - 4998));
    }

    #[tokio_shared_rt::test(shared)]
    async fn write_or_preserve_cache_keeps_existing_ranking_on_empty_graph_result(
    ) -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let timeframe = CacheTimeframe::Today;

        let original = Influencers(vec![("alice".to_string(), 10.0), ("bob".to_string(), 5.0)]);
        Influencers::put_to_global_cache(original, timeframe, TEST_PREFIX).await?;

        Influencers::write_or_preserve_cache(Some(Influencers(vec![])), timeframe, TEST_PREFIX)
            .await?;

        let cached = read_raw_cache(timeframe)
            .await?
            .expect("the previous cache must still exist after an empty graph result");
        assert_eq!(
            cached.len(),
            2,
            "empty graph result must not evict the previous ranking"
        );
        assert!(cached.iter().any(|(id, _)| id == "alice"));
        assert!(cached.iter().any(|(id, _)| id == "bob"));

        clear_test_cache(timeframe).await?;
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn write_or_preserve_cache_keeps_existing_ranking_on_none_graph_result(
    ) -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let timeframe = CacheTimeframe::ThisWeek;

        let original = Influencers(vec![("carol".to_string(), 20.0)]);
        Influencers::put_to_global_cache(original, timeframe, TEST_PREFIX).await?;

        Influencers::write_or_preserve_cache(None, timeframe, TEST_PREFIX).await?;

        let cached = read_raw_cache(timeframe)
            .await?
            .expect("the previous cache must still exist after a None graph result");
        assert_eq!(
            cached.len(),
            1,
            "None graph result must not evict the previous ranking"
        );
        assert!(cached.iter().any(|(id, _)| id == "carol"));

        clear_test_cache(timeframe).await?;
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn write_or_preserve_cache_replaces_existing_ranking_on_non_empty_graph_result(
    ) -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let timeframe = CacheTimeframe::ThisMonth;

        let original = Influencers(vec![("dave".to_string(), 1.0)]);
        Influencers::put_to_global_cache(original, timeframe, TEST_PREFIX).await?;

        let replacement = Influencers(vec![("erin".to_string(), 99.0)]);
        Influencers::write_or_preserve_cache(Some(replacement), timeframe, TEST_PREFIX).await?;

        let cached = read_raw_cache(timeframe)
            .await?
            .expect("a non-empty graph result must leave a cache");
        assert_eq!(
            cached.len(),
            1,
            "non-empty graph result must replace the previous ranking"
        );
        assert!(cached.iter().any(|(id, _)| id == "erin"));
        assert!(!cached.iter().any(|(id, _)| id == "dave"));

        clear_test_cache(timeframe).await?;
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn write_all_time_ranking_keeps_the_previous_ranking_when_the_read_fails(
    ) -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let prefix = "InfluencersAllTimeTest:FailedRead";
        let previous = vec![("alice".to_string(), 10.0)];
        Influencers::write_all_time_ranking(Ok(previous.clone()), Some(prefix)).await?;

        let failed_read = Err(ModelError::from_generic("graph unavailable"));
        let result = Influencers::write_all_time_ranking(failed_read, Some(prefix)).await;

        assert!(result.is_err(), "a failed read must fail the refresh");
        assert_eq!(read_all_time(Some(prefix)).await?, Some(previous));
        Influencers::write_all_time_ranking(Ok(Vec::new()), Some(prefix)).await?;
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn write_all_time_ranking_replaces_members_and_scores() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let prefix = "InfluencersAllTimeTest:Replace";
        let previous = vec![("alice".to_string(), 10.0), ("bob".to_string(), 5.0)];
        Influencers::write_all_time_ranking(Ok(previous), Some(prefix)).await?;

        let ranking = vec![("bob".to_string(), 1.0), ("carol".to_string(), 2.0)];
        Influencers::write_all_time_ranking(Ok(ranking), Some(prefix)).await?;

        // Not additive: alice leaves and bob's score goes down.
        assert_eq!(
            read_all_time(Some(prefix)).await?,
            Some(vec![("carol".to_string(), 2.0), ("bob".to_string(), 1.0)])
        );
        Influencers::write_all_time_ranking(Ok(Vec::new()), Some(prefix)).await?;
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn write_all_time_ranking_deletes_the_key_on_an_empty_ranking() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let prefix = "InfluencersAllTimeTest:Empty";
        let previous = vec![("alice".to_string(), 10.0)];
        Influencers::write_all_time_ranking(Ok(previous), Some(prefix)).await?;

        Influencers::write_all_time_ranking(Ok(Vec::new()), Some(prefix)).await?;

        assert_eq!(
            read_all_time(Some(prefix)).await?,
            None,
            "an empty ranking must not keep the previous members"
        );
        Ok(())
    }

    /// A stale live ranking converges on the graph at the next refresh, with no watcher
    /// update: an inflated score drops, a missing user comes back, and members the graph
    /// does not rank (gone, tombstoned, stubs) leave. Rewrites the shared live set, so
    /// `.config/nextest.toml` runs it alone.
    #[tokio_shared_rt::test(shared)]
    async fn refresh_converges_a_stale_all_time_ranking() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let graph_ranking = Influencers::read_all_time_ranking(ALL_TIME_PAGE_SIZE).await?;
        // The top user scores from every count, so a wrong formula changes their score.
        let (rescored, _) = graph_ranking
            .iter()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .expect("the fixture ranks users");
        let (missing, _) = graph_ranking
            .iter()
            .find(|(id, _)| id != rescored)
            .expect("the fixture ranks two users");

        let gone = PubkyId::from(Keypair::random());
        let tombstone = PubkyId::from(Keypair::random());
        let stub = PubkyId::from(Keypair::random());
        let unranked: [(&str, &str); 3] = [
            (&*gone, "a user gone from the graph"),
            (&*tombstone, "a tombstoned user"),
            (&*stub, "a discovery stub"),
        ];
        let stale: Vec<(f64, &str)> = std::iter::once(rescored.as_str())
            .chain(unranked.iter().map(|(id, _)| *id))
            .map(|id| (1e9, id))
            .collect();
        Influencers::put_index_sorted_set(&USER_INFLUENCERS_KEY_PARTS, &stale, None, None).await?;
        Influencers::remove_from_index_sorted_set(
            None,
            &USER_INFLUENCERS_KEY_PARTS,
            &[missing.as_str()],
        )
        .await?;
        UserDetails::tombstone(&tombstone).put_to_graph().await?;
        UserDetails::from_pubky(stub.clone()).put_to_graph().await?;

        // Remove the nodes before asserting, so a failure cannot leave them in the graph.
        let refreshed = Influencers::refresh(&Timeframe::AllTime).await;
        let live = read_all_time(None).await;
        UserDetails::delete(&tombstone).await?;
        UserDetails::delete(&stub).await?;
        refreshed?;
        let live: HashMap<String, f64> = live?.unwrap_or_default().into_iter().collect();

        let counts = UserCounts::get_by_id(rescored)
            .await?
            .expect("a ranked user has counts");
        let graph_score =
            f64::from(counts.tagged + counts.posts) * f64::from(counts.followers).sqrt();
        assert_eq!(
            live.get(rescored),
            Some(&graph_score),
            "an inflated score must drop to the graph's"
        );
        assert!(
            live.contains_key(missing),
            "a user missing from the ranking must come back"
        );
        for (id, member) in unranked {
            assert!(!live.contains_key(id), "{member} must leave the ranking");
        }
        assert_eq!(live.len(), graph_ranking.len(), "every live user is ranked");
        assert!(
            live.len() > GLOBAL_INFLUENCERS_CACHE_SIZE,
            "the fixture must rank more users than a ranged cache holds, got {}",
            live.len()
        );
        Ok(())
    }

    /// The fixture fits in one page of `ALL_TIME_PAGE_SIZE`, so walk it in small pages
    /// to cover the cursor a larger graph needs.
    #[tokio_shared_rt::test(shared)]
    async fn read_all_time_ranking_walks_every_page() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let page_size = 7;

        let whole = Influencers::read_all_time_ranking(ALL_TIME_PAGE_SIZE).await?;
        let paged = Influencers::read_all_time_ranking(page_size).await?;

        assert!(
            whole.len() > 2 * page_size,
            "the fixture must span several pages"
        );
        assert_eq!(paged, whole);
        Ok(())
    }

    /// Requests that find the AllTime key missing share one rebuild: each waits for the
    /// one in progress, then finds the key instead of reading the whole graph again.
    /// Counts the ranking's replace script in Redis's server-wide command stats, so
    /// `.config/nextest.toml` runs it alone.
    #[tokio_shared_rt::test(shared)]
    async fn requests_that_miss_together_share_one_rebuild() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        // Load the replace script first, so a NOSCRIPT retry cannot add a call below.
        Influencers::refresh(&Timeframe::AllTime).await?;
        Influencers::replace_index_sorted_set(&USER_INFLUENCERS_KEY_PARTS, &[], None, None).await?;
        let requests = 8;
        let scripts_before = evalsha_calls().await?;

        let pages =
            futures::future::try_join_all((0..requests).map(|_| {
                Influencers::get_influencers(None, None, 0, 20, Timeframe::AllTime, false)
            }))
            .await?;

        let rebuilds = evalsha_calls().await? - scripts_before;
        assert_eq!(
            rebuilds, 1,
            "{requests} requests that missed together must share one rebuild"
        );
        assert!(
            pages
                .iter()
                .all(|page| page.as_ref().is_some_and(|p| p.len() == 20)),
            "every request must get a full page"
        );
        Ok(())
    }

    /// The rebuild lock only holds up misses of its own ranking: with the AllTime one
    /// taken, a read that finds the AllTime ranking returns, and so does a request that
    /// misses the ThisWeek cache. Deletes that shared cache key, so nextest runs it alone.
    #[tokio_shared_rt::test(shared)]
    async fn a_rebuild_holds_up_only_misses_of_its_own_ranking() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        Influencers::refresh(&Timeframe::AllTime).await?;
        let this_week = Influencers::get_cache_key_parts(CacheTimeframe::ThisWeek);
        let this_week: Vec<&str> = this_week.iter().map(String::as_str).collect();
        Influencers::replace_index_sorted_set(
            &this_week,
            &[],
            Some(GLOBAL_INFLUENCERS_PREFIX),
            None,
        )
        .await?;
        let _all_time_rebuild = rebuild_lock(&Timeframe::AllTime).lock().await;

        for (timeframe, request) in [
            (Timeframe::AllTime, "a read that finds the AllTime ranking"),
            (
                Timeframe::ThisWeek,
                "a request that misses the ThisWeek cache",
            ),
        ] {
            let read = Influencers::get_influencers(None, None, 0, 5, timeframe, false);
            tokio::time::timeout(Duration::from_secs(5), read)
                .await
                .unwrap_or_else(|_| panic!("{request} must not wait for the AllTime rebuild"))?;
        }
        Ok(())
    }

    /// How many scripts Redis has run by hash (`EVALSHA`), across every client.
    async fn evalsha_calls() -> RedisResult<u64> {
        let mut conn = get_redis_conn().await?;
        let stats: String = redis::cmd("INFO")
            .arg("commandstats")
            .query_async(&mut conn)
            .await?;
        Ok(stats
            .lines()
            .find_map(|line| line.strip_prefix("cmdstat_evalsha:calls="))
            .and_then(|rest| rest.split(',').next())
            .and_then(|calls| calls.parse().ok())
            .unwrap_or(0))
    }

    /// The whole AllTime ranking under `prefix`, the live one for `None`.
    async fn read_all_time(prefix: Option<&str>) -> RedisResult<Option<Vec<(String, f64)>>> {
        Influencers::try_from_index_sorted_set(
            &USER_INFLUENCERS_KEY_PARTS,
            None,
            None,
            Some(0),
            Some(10_000),
            SortOrder::Descending,
            prefix,
        )
        .await
    }

    async fn read_raw_cache(timeframe: CacheTimeframe) -> RedisResult<Option<Vec<(String, f64)>>> {
        let key_parts = Influencers::get_cache_key_parts(timeframe);
        let key_parts_ref: Vec<&str> = key_parts.iter().map(|s| s.as_str()).collect();
        Influencers::try_from_index_sorted_set(
            &key_parts_ref,
            None,
            None,
            Some(0),
            Some(100),
            SortOrder::Descending,
            Some(TEST_PREFIX),
        )
        .await
    }

    async fn clear_test_cache(timeframe: CacheTimeframe) -> RedisResult<()> {
        let key_parts = Influencers::get_cache_key_parts(timeframe);
        let key_parts_ref: Vec<&str> = key_parts.iter().map(|s| s.as_str()).collect();
        Influencers::replace_index_sorted_set(&key_parts_ref, &[], Some(TEST_PREFIX), None).await
    }
}
