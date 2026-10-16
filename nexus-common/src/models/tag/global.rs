use super::{
    stream::{HOT_TAGS_CACHE_PREFIX, POST_HOT_TAGS},
    Taggers as TaggersType,
};
use crate::db::{fetch_key_from_graph, kv::RedisResult, queries, GraphResult, RedisOps};
use crate::models::error::ModelResult;
use crate::types::{StreamReach, Timeframe};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, ops::Deref};
use utoipa::ToSchema;

/// Versioned: `Taggers` holds the pre-[`CachedTaggers`] shape, a bare id array per
/// label, which this type cannot deserialize. A new segment lets those keys age out
/// on their own TTL instead of erroring every read until they do.
const TAGGERS_INDEX: &str = "TaggersV2";

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

/// One label's cached taggers. `total` is the graph's distinct tagger count, which
/// can exceed `taggers.len()`: the list is a sample capped at write time, so it
/// cannot stand in for the count.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CachedTaggers {
    pub taggers: Taggers,
    pub total: usize,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct HotTagsTaggers(pub HashMap<String, CachedTaggers>);

impl RedisOps for HotTagsTaggers {}

impl Deref for HotTagsTaggers {
    type Target = HashMap<String, CachedTaggers>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Taggers {
    /// Cached taggers map for one timeframe
    pub async fn get_from_index(
        timeframe: &Timeframe,
        prefix: &str,
    ) -> RedisResult<Option<HotTagsTaggers>> {
        let timeframe_str = timeframe.to_string();
        HotTagsTaggers::try_from_index_json(
            &Self::build_key_parts(&timeframe_str),
            Some(prefix.into()),
        )
        .await
    }

    /// Overwrites the timeframe's taggers JSON and arms its TTL.
    pub async fn put_to_index(
        taggers: HotTagsTaggers,
        timeframe: &Timeframe,
        prefix: &str,
    ) -> RedisResult<()> {
        let timeframe_str = timeframe.to_string();
        taggers
            .put_index_json(
                &Self::build_key_parts(&timeframe_str),
                Some(prefix.to_string()),
                Some(timeframe.to_cache_period()),
            )
            .await
    }

    /// Global taggers come from the hot-tags cache; reach-scoped taggers hit the graph.
    /// This does not warm the cache: a miss returns `None` until hot tags write it.
    pub async fn get_global_taggers(
        label: String,
        user_id: Option<String>,
        reach: Option<StreamReach>,
        skip: usize,
        limit: usize,
        timeframe: Timeframe,
    ) -> ModelResult<Option<TaggersType>> {
        Ok(match user_id {
            None => Self::get_from_global_timeline(&label, skip, limit, &timeframe).await?,
            Some(id) => {
                Self::get_tag_taggers_by_reach(
                    &label,
                    &id,
                    reach.unwrap_or(StreamReach::Following),
                    skip,
                    limit,
                )
                .await?
            }
        })
    }

    /// Page of taggers for `label` from the cached map, if that timeframe key exists.
    async fn get_from_global_timeline(
        label: &str,
        skip: usize,
        limit: usize,
        timeframe: &Timeframe,
    ) -> RedisResult<Option<TaggersType>> {
        let Some(by_label) = Self::get_from_index(timeframe, HOT_TAGS_CACHE_PREFIX).await? else {
            return Ok(None);
        };
        Ok(by_label
            .get(label)
            .map(|cached| Self::get_taggers_by_pagination(&cached.taggers, skip, limit)))
    }

    /// Slice a cached tagger list. Used by both the taggers route and hot-tag reconstruction.
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

    async fn get_tag_taggers_by_reach(
        label: &str,
        user_id: &str,
        reach: StreamReach,
        skip: usize,
        limit: usize,
    ) -> GraphResult<Option<TaggersType>> {
        let query = queries::get::get_tag_taggers_by_reach(label, user_id, reach, skip, limit);
        fetch_key_from_graph::<TaggersType>(query, "tagger_ids").await
    }

    fn build_key_parts(timeframe: &str) -> Vec<&str> {
        [&POST_HOT_TAGS[..], &[TAGGERS_INDEX], &[timeframe]].concat()
    }
}
