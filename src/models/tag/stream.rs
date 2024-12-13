use crate::db::graph::exec::retrieve_from_graph;
use crate::db::kv::index::sorted_sets::SortOrder;
use crate::types::{DynError, StreamReach, Timeframe};
use axum::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use std::ops::Deref;
use utoipa::ToSchema;

use crate::queries;
use crate::{RedisOps, ScoreAction};

pub const TAG_GLOBAL_HOT: [&str; 3] = ["Tags", "Global", "Hot"];

const GLOBAL_HOT_TAGS_PREFIX: &str = "Cache:Hot:Tags";

#[derive(Serialize, Deserialize, Debug, ToSchema, Clone)]
pub struct Taggers(pub Vec<String>);

#[derive(Deserialize, Debug, ToSchema, Clone)]
pub enum TaggedType {
    Post,
    User,
}

impl Display for TaggedType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaggedType::Post => write!(f, "Post"),
            TaggedType::User => write!(f, "User"),
        }
    }
}

pub struct HotTagsInput {
    pub timeframe: Timeframe,
    pub skip: usize,
    pub limit: usize,
    pub taggers_limit: usize,
    pub tagged_type: Option<TaggedType>,
}

#[async_trait]
impl RedisOps for Taggers {
    async fn prefix() -> String {
        String::from("Tags:Taggers")
    }
}

impl AsRef<[String]> for Taggers {
    fn as_ref(&self) -> &[String] {
        &self.0
    }
}

impl Taggers {
    pub async fn update_index_score(
        label: &str,
        score_action: ScoreAction,
    ) -> Result<(), DynError> {
        Self::put_score_index_sorted_set(&TAG_GLOBAL_HOT, &[label], score_action).await
    }

    pub async fn put_to_index(label: &str, user_id: &str) -> Result<(), DynError> {
        Self::put_index_set(&[label], &[user_id], None, None).await
    }

    pub async fn del_from_index(&self, label: &str) -> Result<(), DynError> {
        self.remove_from_index_set(&[label]).await
    }
}

#[derive(Deserialize, Serialize, ToSchema, Debug, Clone)]
pub struct HotTag {
    label: String,
    taggers_id: Taggers,
    tagged_count: u64,
    taggers_count: usize,
}

// Define a newtype wrapper
#[derive(Serialize, Deserialize, Debug, ToSchema, Default, Clone)]
pub struct HotTags(pub Vec<HotTag>);

impl RedisOps for HotTags {}

#[derive(Serialize, Deserialize, Debug, ToSchema, Default)]
pub struct HotTagsData(pub HashMap<String, HotTag>);

impl RedisOps for HotTagsData {}

// Implement Deref so TagList can be used like Vec<String>
impl Deref for HotTags {
    type Target = Vec<HotTag>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// Create a HotTags instance directly from an iterator of HotTag items
// Need it in collect()
impl FromIterator<HotTag> for HotTags {
    fn from_iter<I: IntoIterator<Item = HotTag>>(iter: I) -> Self {
        HotTags(iter.into_iter().collect())
    }
}

impl HotTags {
    pub async fn reindex() -> Result<(), DynError> {
        HotTags::get_global_hot_tags(&HotTagsInput {
            limit: 100,
            skip: 0,
            taggers_limit: 20,
            timeframe: Timeframe::AllTime,
            tagged_type: Some(TaggedType::Post),
        })
        .await?;
        HotTags::get_global_hot_tags(&HotTagsInput {
            limit: 100,
            skip: 0,
            taggers_limit: 20,
            timeframe: Timeframe::ThisMonth,
            tagged_type: Some(TaggedType::Post),
        })
        .await?;
        Ok(())
    }

    fn get_cache_key_parts(tags_query: &HotTagsInput) -> Vec<String> {
        match &tags_query.tagged_type {
            Some(tagged) => vec![tags_query.timeframe.to_string(), tagged.to_string()],
            None => vec![tags_query.timeframe.to_string(), String::from("All")],
        }
    }

    async fn get_from_global_cache(tags_query: &HotTagsInput) -> Result<Option<HotTags>, DynError> {
        let key_parts = HotTags::get_cache_key_parts(tags_query);
        let key_parts_vector: Vec<&str> =
            key_parts.iter().map(|s| s.as_str()).collect::<Vec<&str>>();
        let data = HotTagsData::try_from_index_json(key_parts_vector.clone().as_slice()).await?;
        let ranking = HotTags::try_from_index_sorted_set(
            key_parts_vector.as_slice(),
            None,
            None,
            Some(tags_query.skip),
            Some(tags_query.limit),
            SortOrder::Descending,
            Some(GLOBAL_HOT_TAGS_PREFIX),
        )
        .await?;

        if data.is_none() || ranking.is_none() {
            return Ok(None);
        }

        let mapping = data.unwrap();
        // for each value in ranking, look up the value in data
        let mut hot_tags = Vec::new();
        for (label, _) in ranking.unwrap() {
            if let Some(tag) = mapping.0.get(&label) {
                hot_tags.push(HotTag {
                    label,
                    taggers_id: Taggers(
                        tag.taggers_id
                            .0
                            .clone()
                            .into_iter()
                            .take(tags_query.taggers_limit)
                            .collect(),
                    ),
                    tagged_count: tag.tagged_count,
                    taggers_count: tag.taggers_count,
                });
            }
        }
        Ok(Some(HotTags(hot_tags)))
    }

    async fn set_to_global_cache(
        result: HotTags,
        tags_query: &HotTagsInput,
    ) -> Result<(), DynError> {
        let key_parts = HotTags::get_cache_key_parts(tags_query);
        let key_parts_vector: Vec<&str> =
            key_parts.iter().map(|s| s.as_str()).collect::<Vec<&str>>();

        // turn result which is a vector of HotTag, into a mapping from label to HotTag
        let mapping = result.clone();
        let hot_tags_data: HashMap<String, HotTag> = mapping
            .iter()
            .cloned()
            .map(|tag| (tag.label.clone(), tag))
            .collect();

        // store the data as json in cache
        HotTagsData::put_index_json(
            &HotTagsData(hot_tags_data),
            key_parts_vector.as_slice(),
            Some(tags_query.timeframe.to_cache_period()),
        )
        .await?;

        // store the ranking as sorted set in cache
        HotTags::put_index_sorted_set(
            key_parts_vector.as_slice(),
            result
                .iter()
                .map(|tag| (tag.tagged_count as f64, tag.label.as_str()))
                .collect::<Vec<(f64, &str)>>()
                .as_slice(),
            Some(GLOBAL_HOT_TAGS_PREFIX),
            Some(tags_query.timeframe.to_cache_period()),
        )
        .await?;
        Ok(())
    }

    pub async fn get_hot_tags(
        user_id: Option<String>,
        reach: Option<StreamReach>,
        tags_query: &HotTagsInput,
    ) -> Result<Option<HotTags>, DynError> {
        match user_id {
            Some(user_id) => {
                HotTags::get_hot_tags_by_reach(user_id, reach.unwrap(), tags_query).await
            }
            None => HotTags::get_global_hot_tags(tags_query).await,
        }
    }

    async fn get_hot_tags_by_reach(
        user_id: String,
        reach: StreamReach,
        tags_query: &HotTagsInput,
    ) -> Result<Option<HotTags>, DynError> {
        let query = queries::get::get_hot_tags_by_reach(user_id.as_str(), reach, tags_query);
        retrieve_from_graph::<HotTags>(query, "hot_tags").await
    }

    async fn get_global_hot_tags(tags_query: &HotTagsInput) -> Result<Option<HotTags>, DynError> {
        let cached_hot_tags = HotTags::get_from_global_cache(tags_query).await?;
        if cached_hot_tags.is_some() {
            return Ok(cached_hot_tags);
        }

        let query = queries::get::get_global_hot_tags(&HotTagsInput {
            skip: 0,
            limit: 100,
            tagged_type: tags_query.tagged_type.clone(),
            taggers_limit: 20,
            timeframe: tags_query.timeframe.clone(),
        });
        let result = retrieve_from_graph::<HotTags>(query, "hot_tags").await?;

        let hot_tags = result.unwrap();
        if hot_tags.len() > 0 {
            HotTags::set_to_global_cache(hot_tags.clone(), tags_query).await?;
        }

        HotTags::get_from_global_cache(tags_query).await
    }
}
