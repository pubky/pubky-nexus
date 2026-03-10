use crate::db::kv::{RedisResult, SortOrder};
use crate::db::{fetch_key_from_graph, queries, RedisOps};
use crate::models::error::ModelResult;
use crate::types::routes::HotTagsInputDTO;
use crate::types::{StreamReach, Timeframe};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Deref;
use utoipa::ToSchema;

use super::global::{HotTagsTaggers, Taggers};
use super::TaggedType;

pub const HOT_TAGS_CACHE_PREFIX: &str = "Cache";
pub const POST_HOT_TAGS: [&str; 3] = ["Tags", "Post", "Hot"];

#[derive(Deserialize, Serialize, ToSchema, Debug, Clone)]
pub struct HotTag {
    pub label: String,
    pub taggers_id: Taggers,
    pub tagged_count: u64,
    pub taggers_count: usize,
}

// Define a newtype wrapper
#[derive(Serialize, Deserialize, Debug, ToSchema, Default, Clone)]
pub struct HotTags(pub Vec<HotTag>);

impl RedisOps for HotTags {}

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
    /// It dynamically determines whether to fetch **global hot tags** or **user-specific hot tags**
    /// based on the provided `user_id` and `reach` parameters
    ///
    /// # Arguments
    /// * `user_id` - An optional user ID
    /// * `reach` - An optional `TagStreamReach` value specifying the scope of tag retrieval
    /// * `hot_tags_input` - The input parameters received from the API endpoint
    pub async fn get_hot_tags(
        user_id: Option<String>,
        reach: Option<StreamReach>,
        hot_tags_input: &HotTagsInputDTO,
    ) -> ModelResult<Option<HotTags>> {
        match user_id {
            Some(user_id) => {
                HotTags::get_hot_tags_by_reach(
                    user_id,
                    reach.unwrap_or(StreamReach::Following),
                    hot_tags_input,
                )
                .await
            }
            None => HotTags::get_global_hot_tags(hot_tags_input).await,
        }
    }

    /// Retrieves hot tags based on the user's reach criteria
    /// Queries the graph database to fetch hot tags relevant to a given user,
    /// filtered by their reach and additional criteria defined in `hot_tags_input`
    ///
    /// # Arguments
    /// * `user_id` - The ID of the user whose reach is used for filtering hot tags
    /// * `reach` - The `TagStreamReach` parameter that defines the scope of tag retrieval
    /// * `hot_tags_input` - The input parameters received from the API endpoint
    async fn get_hot_tags_by_reach(
        user_id: String,
        reach: StreamReach,
        hot_tags_input: &HotTagsInputDTO,
    ) -> ModelResult<Option<HotTags>> {
        let query = queries::get::get_hot_tags_by_reach(user_id.as_str(), reach, hot_tags_input);
        fetch_key_from_graph::<HotTags>(query, "hot_tags")
            .await
            .map_err(Into::into)
    }

    /// Retrieves global hot tags, checking the cache first before querying the database.
    /// This function first attempts to fetch global hot tags from the cache. If the cached
    /// data is unavailable, it queries the graph database to retrieve the latest hot tags.
    /// If new data is found, it updates the cache before returning the results.
    ///
    /// # Arguments
    ///
    /// * `hot_tags_input` - The input parameters received from the API endpoint
    async fn get_global_hot_tags(hot_tags_input: &HotTagsInputDTO) -> ModelResult<Option<HotTags>> {
        let cached_hot_tags = HotTags::get_from_global_cache(hot_tags_input).await?;

        if let Some(hot_tags) = &cached_hot_tags {
            if hot_tags.0.is_empty() {
                return Ok(None);
            }
            return Ok(cached_hot_tags);
        }

        let hot_tag_input = HotTagsInputDTO::new(
            hot_tags_input.timeframe.clone(),
            100,
            0,
            20,
            hot_tags_input.tagged_type.clone(),
        );
        let query = queries::get::get_global_hot_tags(&hot_tag_input);
        let result = fetch_key_from_graph::<HotTags>(query, "hot_tags").await?;

        let hot_tags = match result {
            Some(hot_tags) => hot_tags,
            None => return Ok(None),
        };
        if !hot_tags.is_empty() {
            HotTags::set_to_global_cache(hot_tags.clone(), hot_tags_input).await?;
        }

        HotTags::get_from_global_cache(hot_tags_input)
            .await
            .map_err(Into::into)
    }

    /// Retrieves hot tags from the global cache
    ///
    /// Fetches hot tags and their associated taggers from the cache, reconstructing
    /// a list of hot tags from a stored JSON mapping and a hot tags SORTED SET. It applies filters
    /// based on `hot_tags_input`, ensuring that only relevant tags and taggers are returned
    ///
    /// # Arguments
    ///
    /// * `hot_tags_input` - The input parameters received from the API endpoint
    async fn get_from_global_cache(
        hot_tags_input: &HotTagsInputDTO,
    ) -> RedisResult<Option<HotTags>> {
        let timeframe = hot_tags_input.timeframe.to_string();
        let hot_tag_key_parts = Self::build_hot_tags_key_parts(&timeframe);

        let hot_tag_taggers = Taggers::get_from_index(&timeframe).await?;

        let hot_tags_score = HotTags::try_from_index_sorted_set(
            &hot_tag_key_parts,
            None,
            None,
            Some(hot_tags_input.skip),
            Some(hot_tags_input.limit),
            SortOrder::Descending,
            Some(HOT_TAGS_CACHE_PREFIX),
        )
        .await?;

        let (hot_tags_score, hot_tag_taggers) = match (hot_tags_score, hot_tag_taggers) {
            (Some(score_list), Some(taggers)) => {
                // Index exist but applyting the DTO filters, there is not records
                if score_list.is_empty() {
                    return Ok(Some(HotTags(Vec::new())));
                }
                (score_list, taggers)
            }
            _ => return Ok(None),
        };

        let mut hot_tags = Vec::with_capacity(hot_tags_score.len());

        for (label, score) in hot_tags_score {
            if let Some(taggers) = hot_tag_taggers.get(&label) {
                // Reduce taggers list
                let taggers_id: Vec<String> =
                    Taggers::get_taggers_by_pagination(taggers, 0, hot_tags_input.taggers_limit);
                hot_tags.push(HotTag {
                    label,
                    taggers_id: Taggers(taggers_id),
                    tagged_count: score as u64,
                    taggers_count: taggers.len(),
                });
            }
        }
        Ok(Some(HotTags(hot_tags)))
    }

    /// Caches the global hot tags taggers and their scores
    /// Gets hot tags and stores it in a global cache, both as a JSON
    /// mapping of taggers and as a sorted set for score. It constructs cache keys dynamically
    /// based on the provided timeframe
    ///
    /// # Arguments
    ///
    /// * `hot_tags_list` - A vector of `HotTag` elements
    /// * `hot_tags_input` - The input parameters received from the API endpoint
    async fn set_to_global_cache(
        hot_tags_list: HotTags,
        hot_tags_input: &HotTagsInputDTO,
    ) -> RedisResult<()> {
        let timeframe = hot_tags_input.timeframe.to_string();
        let hot_tag_key_parts = Self::build_hot_tags_key_parts(&timeframe);

        let mut hot_tags_score = Vec::with_capacity(hot_tags_list.len());

        let taggers: HashMap<String, Taggers> = hot_tags_list
            .iter()
            .map(|tag| {
                hot_tags_score.push((tag.tagged_count as f64, tag.label.as_str()));
                (tag.label.clone(), tag.taggers_id.clone())
            })
            .collect();

        Taggers::put_to_index(HotTagsTaggers(taggers), &hot_tags_input.timeframe).await?;

        // Store the score as sorted set in cache
        HotTags::put_index_sorted_set(
            &hot_tag_key_parts,
            &hot_tags_score,
            Some(HOT_TAGS_CACHE_PREFIX),
            Some(hot_tags_input.timeframe.to_cache_period()),
        )
        .await
    }

    /// Builds key parts for hot tags based on the given timeframe
    ///
    /// # Arguments
    /// * `timeframe` - A string slice representing the timeframe (e.g., "today", "this_month", "all_time")
    fn build_hot_tags_key_parts(timeframe: &str) -> Vec<&str> {
        [&POST_HOT_TAGS[..], &[timeframe]].concat()
    }

    /// Reindexes global hot tags
    /// Retrieves and updates global hot tags for different timeframes. It fetches the top 100 hot tags
    ///  with a taggers limit of 20 for both "all-time" and "this month" timeframes
    pub async fn reindex() -> ModelResult<()> {
        let all_timeframe_input =
            HotTagsInputDTO::new(Timeframe::AllTime, 100, 0, 20, Some(TaggedType::Post));
        HotTags::get_global_hot_tags(&all_timeframe_input).await?;

        let month_timeframe_input =
            HotTagsInputDTO::new(Timeframe::ThisMonth, 100, 0, 20, Some(TaggedType::Post));
        HotTags::get_global_hot_tags(&month_timeframe_input).await?;
        Ok(())
    }
}
