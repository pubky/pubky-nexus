use super::{
    stream::{HOT_TAGS_CACHE_PREFIX, POST_HOT_TAGS},
    Taggers as TaggersType,
};
use crate::db::kv::RedisResult;
use crate::db::{fetch_key_from_graph, queries, RedisOps};
use crate::types::StreamReach;
use crate::types::{DynError, Timeframe};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, ops::Deref};
use utoipa::ToSchema;

const TAGGERS_INDEX: &str = "Taggers";

#[derive(Serialize, Deserialize, Debug, ToSchema, Clone)]
pub struct Taggers(pub TaggersType);

impl Deref for Taggers {
    type Target = TaggersType;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[async_trait]
impl RedisOps for Taggers {}

impl AsRef<[String]> for Taggers {
    fn as_ref(&self) -> &[String] {
        &self.0
    }
}

#[derive(Serialize, Deserialize, Debug, ToSchema, Default)]
pub struct HotTagsTaggers(pub HashMap<String, Taggers>);

impl RedisOps for HotTagsTaggers {}

// Implement Deref so TagList can be used like Vec<String>
impl Deref for HotTagsTaggers {
    type Target = HashMap<String, Taggers>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Taggers {
    /// Retrieves taggers from the cache for a given timeframe
    ///
    /// # Arguments
    /// * `timeframe` - A string representing the timeframe for which to retrieve taggers
    pub async fn get_from_index(timeframe: &str) -> RedisResult<Option<HotTagsTaggers>> {
        let key_parts = Self::build_key_parts(timeframe);
        HotTagsTaggers::try_from_index_json(&key_parts, Some(HOT_TAGS_CACHE_PREFIX.to_string()))
            .await
    }

    /// Stores taggers in the cache for a given timeframe
    ///
    /// # Arguments
    /// * `taggers` - The collection of taggers to be stored
    /// * `timeframe` - The timeframe for which the taggers are indexed, determining the cache key and expiration period
    pub async fn put_to_index(taggers: HotTagsTaggers, timeframe: &Timeframe) -> RedisResult<()> {
        let timeframe_str = timeframe.to_string();
        let key_parts = Self::build_key_parts(&timeframe_str);

        // Store the taggers as JSON in cache
        taggers
            .put_index_json(
                key_parts.as_slice(),
                Some(HOT_TAGS_CACHE_PREFIX.to_string()),
                Some(timeframe.to_cache_period()),
            )
            .await
    }

    /// Retrieves taggers for a given tag label, either globally or based on a user's reach
    ///
    /// # Arguments
    /// * `label` - The tag label for which to retrieve taggers
    /// * `user_id` - An optional user ID. If provided, taggers are retrieved based on the user's reach.
    /// * `reach` - An optional reach context (e.g., friends, followers, following). Defaults to `Following`
    /// * `skip` - The number of taggers to skip for pagination
    /// * `limit` - The maximum number of taggers to retrieve
    /// * `timeframe` - The timeframe within which to search for taggers (e.g., Today, ThisMonth, AllTime).
    pub async fn get_global_taggers(
        label: String,
        user_id: Option<String>,
        reach: Option<StreamReach>,
        skip: usize,
        limit: usize,
        timeframe: Timeframe,
    ) -> Result<Option<TaggersType>, DynError> {
        match user_id {
            None => Self::get_from_global_timeline(&label, skip, limit, timeframe).await,
            Some(id) => {
                Self::get_tag_taggers_by_reach(
                    &label,
                    &id,
                    reach.unwrap_or(StreamReach::Following),
                    skip,
                    limit,
                )
                .await
            }
        }
    }

    /// Retrieves paginated taggers from the global timeline based on a specified timeframe
    ///
    /// # Arguments
    /// * `label` - The tag label for which to retrieve taggers
    /// * `skip` - The number of taggers to skip for pagination
    /// * `limit` - The maximum number of taggers to retrieve
    /// * `timeframe` - The timeframe within which to search for taggers (e.g., Today, ThisMonth, AllTime)
    async fn get_from_global_timeline(
        label: &str,
        skip: usize,
        limit: usize,
        timeframe: Timeframe,
    ) -> Result<Option<TaggersType>, DynError> {
        let timeframe_str = timeframe.to_string();
        let taggers_by_timeframe = Self::get_from_index(&timeframe_str).await?;

        if let Some(taggers_hash_map) = taggers_by_timeframe {
            if let Some(taggers) = taggers_hash_map.get(label) {
                return Ok(Some(Self::get_taggers_by_pagination(taggers, skip, limit)));
            }
        }

        Ok(None)
    }

    /// Returns a paginated subset of taggers from the given list
    ///
    /// # Arguments
    /// * `taggers_list` - A reference to the list of taggers to paginate
    /// * `skip` - The number of initial elements to skip
    /// * `limit` - The maximum number of taggers to return after skipping
    pub fn get_taggers_by_pagination(
        taggers_list: &Taggers,
        skip: usize,
        limit: usize,
    ) -> TaggersType {
        taggers_list
            .iter()
            .skip(skip)
            .take(limit)
            .cloned()
            .collect()
    }

    /// Retrieves taggers associated with a given tag label based on the user's reach.
    ///
    /// # Arguments
    /// * `label` - The tag label for which to retrieve taggers
    /// * `user_id` - The ID of the user whose reach is used for filtering taggers
    /// * `reach` - The reach context that determines the visibility of taggers (e.g., followers, following, friends)
    /// * `skip` - The number of taggers to skip for pagination
    /// * `limit` - The maximum number of taggers to retrieve
    async fn get_tag_taggers_by_reach(
        label: &str,
        user_id: &str,
        reach: StreamReach,
        skip: usize,
        limit: usize,
    ) -> Result<Option<TaggersType>, DynError> {
        let query = queries::get::get_tag_taggers_by_reach(label, user_id, reach, skip, limit);
        fetch_key_from_graph::<TaggersType>(query, "tagger_ids").await
    }

    /// Builds key parts for hot tag taggers based on the given timeframe
    ///
    /// # Arguments
    /// * `timeframe` - A string slice representing the timeframe (e.g., "today", "this_month", "all_time")
    fn build_key_parts(timeframe: &str) -> Vec<&str> {
        [&POST_HOT_TAGS[..], &[TAGGERS_INDEX], &[timeframe]].concat()
    }
}
