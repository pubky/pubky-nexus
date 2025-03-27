use crate::db::kv::SortOrder;
use crate::types::DynError;
use crate::types::StreamReach;
use crate::types::Timeframe;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use utoipa::ToSchema;

use crate::db::{queries, retrieve_from_graph, RedisOps};

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
        timeframe: &Timeframe,
        preview: bool,
    ) -> Result<Option<Influencers>, DynError> {
        let (skip, limit) = if preview {
            // get a pseudo-random number between 0 and 97
            let skip = Utc::now().timestamp_subsec_micros() % 98;
            (skip as usize, 3)
        } else {
            (skip, limit)
        };
        match user_id {
            Some(user_id) => {
                Influencers::get_influencers_by_reach(
                    user_id,
                    reach.unwrap_or(StreamReach::Friends),
                    skip,
                    limit,
                    timeframe,
                )
                .await
            }
            None => Influencers::get_global_influencers(skip, limit, timeframe).await,
        }
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
    ) -> Result<Option<Influencers>, DynError> {
        let cached_influencers = Influencers::get_from_global_cache(skip, limit, timeframe).await?;
        if cached_influencers.is_some() {
            return Ok(cached_influencers);
        }

        let query = queries::get::get_global_influencers(0, 100, timeframe);
        let result = retrieve_from_graph::<Influencers>(query, "influencers").await?;

        let influencers = match result {
            Some(influencers) => influencers,
            None => return Ok(None),
        };

        if !influencers.is_empty() {
            Influencers::put_to_global_cache(influencers.clone(), timeframe).await?;
        }

        Influencers::get_from_global_cache(skip, limit, timeframe).await
    }

    /// Retrieves a paginated list of global influencers from the cache for the given timeframe
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
    ) -> Result<Option<Influencers>, DynError> {
        let key_parts = Influencers::get_cache_key_parts(timeframe);
        let key_parts_vector: Vec<&str> =
            key_parts.iter().map(|s| s.as_str()).collect::<Vec<&str>>();
        let ranking = Influencers::try_from_index_sorted_set(
            key_parts_vector.as_slice(),
            None,
            None,
            Some(skip),
            Some(limit),
            SortOrder::Descending,
            Some(GLOBAL_INFLUENCERS_PREFIX),
        )
        .await?;

        match ranking {
            None => Ok(None),
            Some(ranking) => Ok(Some(Influencers(ranking))),
        }
    }

    /// Stores a list of global influencers in the cache as a sorted set for the given timeframe
    ///
    /// # Arguments
    /// * `result` - The list of influencers with their scores to cache
    /// * `timeframe` - The timeframe used to generate the cache key and expiry
    async fn put_to_global_cache(
        result: Influencers,
        timeframe: &Timeframe,
    ) -> Result<(), DynError> {
        let key_parts = Influencers::get_cache_key_parts(timeframe);
        let key_parts_vector: Vec<&str> =
            key_parts.iter().map(|s| s.as_str()).collect::<Vec<&str>>();

        // store the ranking as sorted set in cache
        Influencers::put_index_sorted_set(
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
    ) -> Result<Option<Influencers>, DynError> {
        let query = queries::get::get_influencers_by_reach(user_id, reach, skip, limit, timeframe);
        retrieve_from_graph::<Influencers>(query, "influencers").await
    }

    fn get_cache_key_parts(timeframe: &Timeframe) -> Vec<String> {
        vec![timeframe.to_string()]
    }

    /// Rebuilds the global influencer cache for `AllTime` and `ThisMonth` timeframes
    ///
    pub async fn reindex() -> Result<(), DynError> {
        Influencers::get_global_influencers(0, 100, &Timeframe::AllTime).await?;
        Influencers::get_global_influencers(0, 100, &Timeframe::ThisMonth).await?;
        Ok(())
    }
}
