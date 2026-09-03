use crate::db::kv::RedisResult;
use crate::db::kv::SortOrder;
use crate::models::error::{ModelError, ModelResult};
use crate::types::StreamReach;
use crate::types::Timeframe;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use tracing::{debug, error, warn};
use utoipa::ToSchema;

use super::{UserDetails, USER_DELETED_SENTINEL, USER_INFLUENCERS_KEY_PARTS};
use crate::db::{fetch_key_from_graph, queries, RedisOps};

const GLOBAL_INFLUENCERS_PREFIX: &str = "Cache:Influencers";

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
            // Generate a pseudo-random number between 0 and 97
            // We cache 100 influencers, and pick 3 starting from this number
            // Using modulo 98 ensures we always have room for 3 without going out of bounds
            let skip = Utc::now().timestamp_subsec_micros() % 98;
            debug!("Influencer preview active: skip number {}", skip);
            (skip as usize, 3)
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
    /// based on the provided `skip` and `limit`. If the cache is empty or unavailable,
    /// it queries the graph database for up to 100 global influencers, stores the result
    /// in cache, and then retrieves the requested subset again from cache.
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

        Influencers::fetch_and_cache(timeframe).await?;
        Influencers::get_from_global_cache(skip, limit, timeframe)
            .await
            .map_err(Into::into)
    }

    /// Fetch top-100 influencers from the graph and atomically replace the cache.
    ///
    /// A transient empty or `None` graph result must not evict a good ranking, so the
    /// previous cache is preferred. The caller can retry on the next scheduled tick.
    pub async fn fetch_and_cache(timeframe: &Timeframe) -> ModelResult<()> {
        let query = queries::get::get_global_influencers(0, 100, timeframe);
        let result = fetch_key_from_graph::<Influencers>(query, "influencers").await?;
        Influencers::write_or_preserve_cache(result, timeframe).await
    }

    /// Writes a non-empty graph result to the cache, or preserves the existing cache
    /// when the result is empty or missing. This is the production half of the
    /// `fetch_and_cache` seam; tests drive it directly with injected results.
    async fn write_or_preserve_cache(
        result: Option<Influencers>,
        timeframe: &Timeframe,
    ) -> ModelResult<()> {
        match result {
            Some(influencers) if !influencers.is_empty() => {
                debug!(
                    ?timeframe,
                    count = influencers.len(),
                    "Writing influencer cache"
                );
                Influencers::put_to_global_cache(influencers, timeframe).await?;
            }
            Some(empty) => {
                warn!(
                    ?timeframe,
                    count = empty.len(),
                    "Graph returned empty influencer set — previous cache left untouched"
                );
            }
            None => {
                warn!(
                    ?timeframe,
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
            // When timeframe is AllTime, we get the influencer list directly from Sorted::Users::Influencers,
            // which is dynamically updated with each user action and therefore needs no TTL.
            // Had we used the cache with TTL, it would have meant a random user gets hit with
            // a full graph lookup, if they query this right after the TTL expires.
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
                let key_parts = Influencers::get_cache_key_parts(timeframe);
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

        match ranking {
            Some(r) => Influencers::filter_deleted(Influencers(r), Some(timeframe)).await,
            None => Ok(None),
        }
    }

    /// Stores a list of global influencers in the cache as a sorted set for the given timeframe
    ///
    /// # Arguments
    /// * `result` - The list of influencers with their scores to cache
    /// * `timeframe` - The timeframe used to generate the cache key and expiry
    async fn put_to_global_cache(result: Influencers, timeframe: &Timeframe) -> ModelResult<()> {
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
            Some(GLOBAL_INFLUENCERS_PREFIX),
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
    ) -> RedisResult<Option<Influencers>> {
        let ids: Vec<String> = influencers.iter().map(|(id, _)| id.clone()).collect();
        let details_list = UserDetails::mget(&ids).await?;

        let mut kept = Vec::new();
        let mut deleted_ids = Vec::new();

        for ((id, score), details) in influencers.0.into_iter().zip(details_list) {
            match details {
                Some(ref d) if d.name != USER_DELETED_SENTINEL => kept.push((id, score)),
                _ => deleted_ids.push(id),
            }
        }

        if let Some(tf) = timeframe {
            Influencers::remove_deleted_from_global_cache(&deleted_ids, tf).await;
        }

        Ok(if kept.is_empty() {
            None
        } else {
            Some(Influencers(kept))
        })
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
                let key_parts = Influencers::get_cache_key_parts(timeframe);
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

    fn get_cache_key_parts(timeframe: &Timeframe) -> Vec<String> {
        vec![timeframe.to_string()]
    }

    /// Run a per-timeframe refresh for `timeframes` and aggregate failures.
    ///
    /// `refresh` is invoked once per timeframe with an owned `Timeframe`. Callers may
    /// wrap the future in a timeout or inject test doubles before passing it in.
    pub async fn refresh_timeframes_with<F, Fut>(
        timeframes: &[Timeframe],
        refresh: &F,
    ) -> ModelResult<()>
    where
        F: Fn(Timeframe) -> Fut,
        Fut: std::future::Future<Output = ModelResult<()>>,
    {
        let mut failed: Vec<String> = Vec::new();
        let mut first_error: Option<ModelError> = None;

        for tf in timeframes {
            let tf = tf.clone();
            let tf_label = tf.to_string();
            if let Err(e) = refresh(tf).await {
                error!(
                    timeframe = %tf_label,
                    error = ?e,
                    "Influencer cache refresh failed"
                );
                first_error.get_or_insert(e);
                failed.push(tf_label);
            }
        }

        if failed.is_empty() {
            Ok(())
        } else {
            let message = format!(
                "{}/{} influencer cache refreshes failed: {}",
                failed.len(),
                timeframes.len(),
                failed.join(", ")
            );
            Err(ModelError::from_generic_with_source(
                message,
                first_error.expect("first_error is Some when failed is not empty"),
            ))
        }
    }

    /// Rebuilds the global influencer cache for `AllTime` and `ThisMonth` timeframes
    pub async fn reindex() -> ModelResult<()> {
        Influencers::get_global_influencers(0, 100, &Timeframe::AllTime).await?;
        Influencers::get_global_influencers(0, 100, &Timeframe::ThisMonth).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{types::DynError, StackConfig, StackManager};
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn refresh_timeframes_with_refreshes_each_passed_timeframe_once() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_ref = seen.clone();

        Influencers::refresh_timeframes_with(
            &[Timeframe::ThisWeek, Timeframe::ThisMonth],
            &|tf: Timeframe| {
                let seen_ref = seen_ref.clone();
                async move {
                    seen_ref.lock().unwrap().push(tf);
                    Ok(())
                }
            },
        )
        .await
        .expect("all refreshes should succeed");

        assert_eq!(
            *seen.lock().unwrap(),
            vec![Timeframe::ThisWeek, Timeframe::ThisMonth],
            "each passed timeframe must be refreshed exactly once, in order"
        );
    }

    #[tokio::test]
    async fn refresh_timeframes_with_aggregates_failures() {
        let err = Influencers::refresh_timeframes_with(
            &[Timeframe::Today],
            &|tf: Timeframe| async move { Err(ModelError::from_generic(format!("boom {tf}"))) },
        )
        .await
        .expect_err("a failing refresh should bubble up");

        let text = err.to_string();
        assert!(
            text.contains("1/1 influencer cache refreshes failed"),
            "error must report the failure ratio, got: {text}"
        );
        assert!(
            text.contains("Today"),
            "error must name the failing timeframe, got: {text}"
        );
    }

    #[tokio::test]
    async fn refresh_timeframes_with_names_all_failed_timeframes_and_preserves_first_source() {
        let err = Influencers::refresh_timeframes_with(
            &[Timeframe::Today, Timeframe::ThisMonth],
            &|tf: Timeframe| async move { Err(ModelError::from_generic(format!("fail-{tf}"))) },
        )
        .await
        .expect_err("all refreshes failed so the aggregate must error");

        let text = err.to_string();
        assert!(
            text.contains("2/2 influencer cache refreshes failed"),
            "error must report the failure ratio, got: {text}"
        );
        assert!(
            text.contains("Today") && text.contains("ThisMonth"),
            "error must name every failed timeframe, got: {text}"
        );

        let first_source = match err {
            ModelError::Generic {
                source: Some(source),
                ..
            } => source,
            other => panic!("aggregate error must be Generic with a source, got: {other:?}"),
        };
        assert!(
            first_source.to_string().contains("fail-Today"),
            "source chain must carry the first failing timeframe's error, got: {first_source}"
        );
    }

    #[tokio_shared_rt::test(shared)]
    async fn write_or_preserve_cache_keeps_existing_ranking_on_empty_graph_result(
    ) -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let timeframe = Timeframe::Today;

        let original = Influencers(vec![("alice".to_string(), 10.0), ("bob".to_string(), 5.0)]);
        Influencers::put_to_global_cache(original, &timeframe).await?;

        Influencers::write_or_preserve_cache(Some(Influencers(vec![])), &timeframe).await?;

        let cached = read_raw_cache(&timeframe)
            .await?
            .expect("the previous cache must still exist after an empty graph result");
        assert_eq!(
            cached.len(),
            2,
            "empty graph result must not evict the previous ranking"
        );
        assert!(cached.iter().any(|(id, _)| id == "alice"));
        assert!(cached.iter().any(|(id, _)| id == "bob"));
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn write_or_preserve_cache_keeps_existing_ranking_on_none_graph_result(
    ) -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let timeframe = Timeframe::ThisWeek;

        let original = Influencers(vec![("carol".to_string(), 20.0)]);
        Influencers::put_to_global_cache(original, &timeframe).await?;

        Influencers::write_or_preserve_cache(None, &timeframe).await?;

        let cached = read_raw_cache(&timeframe)
            .await?
            .expect("the previous cache must still exist after a None graph result");
        assert_eq!(
            cached.len(),
            1,
            "None graph result must not evict the previous ranking"
        );
        assert!(cached.iter().any(|(id, _)| id == "carol"));
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn write_or_preserve_cache_replaces_existing_ranking_on_non_empty_graph_result(
    ) -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let timeframe = Timeframe::ThisMonth;

        let original = Influencers(vec![("dave".to_string(), 1.0)]);
        Influencers::put_to_global_cache(original, &timeframe).await?;

        let replacement = Influencers(vec![("erin".to_string(), 99.0)]);
        Influencers::write_or_preserve_cache(Some(replacement), &timeframe).await?;

        let cached = read_raw_cache(&timeframe)
            .await?
            .expect("a non-empty graph result must leave a cache");
        assert_eq!(
            cached.len(),
            1,
            "non-empty graph result must replace the previous ranking"
        );
        assert!(cached.iter().any(|(id, _)| id == "erin"));
        assert!(!cached.iter().any(|(id, _)| id == "dave"));
        Ok(())
    }

    async fn read_raw_cache(timeframe: &Timeframe) -> RedisResult<Option<Vec<(String, f64)>>> {
        let key_parts = Influencers::get_cache_key_parts(timeframe);
        let key_parts_ref: Vec<&str> = key_parts.iter().map(|s| s.as_str()).collect();
        Influencers::try_from_index_sorted_set(
            &key_parts_ref,
            None,
            None,
            Some(0),
            Some(100),
            SortOrder::Descending,
            Some(GLOBAL_INFLUENCERS_PREFIX),
        )
        .await
    }
}
