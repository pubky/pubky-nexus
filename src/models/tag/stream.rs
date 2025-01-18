use crate::db::graph::exec::retrieve_from_graph;
use crate::db::kv::index::sorted_sets::SortOrder;
use crate::routes::v0::tag::HotTagsInput;
use crate::types::{DynError, Timeframe};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Deref;
use utoipa::ToSchema;

use crate::queries;
use crate::RedisOps;

use super::taggers::Taggers;
use super::TaggedType;

const HOT_TAGS_CACHE_PREFIX: &str = "Cache";
const POST_HOT_TAGS: [&str; 3] = ["Tags", "Post", "Hot"];
const TAGGERS: &str = "Taggers";

#[derive(Deserialize, Debug, ToSchema, Clone)]
#[serde(rename_all = "snake_case")]
pub enum TagStreamReach {
    Followers,
    Following,
    Friends,
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
        reach: Option<TagStreamReach>,
        hot_tags_input: &HotTagsInput,
    ) -> Result<Option<HotTags>, DynError> {
        match user_id {
            Some(user_id) => {
                HotTags::get_hot_tags_by_reach(
                    user_id,
                    reach.unwrap_or(TagStreamReach::Friends),
                    hot_tags_input,
                )
                .await
            }
            None => HotTags::get_global_hot_tags(hot_tags_input).await,
        }
    }

    /// Retrieves hot tags based on the user's reach criteria
    /// Queries the graph database to fetch hot tags relevant to a given user,
    /// filtered by their reach and additional criteria defined in `hot_tags_input`.
    ///
    /// # Arguments
    /// * `user_id` - The ID of the user whose reach is used for filtering hot tags.
    /// * `reach` - The `TagStreamReach` parameter that defines the scope of tag retrieval
    /// * `hot_tags_input` - The input parameters received from the API endpoint
    async fn get_hot_tags_by_reach(
        user_id: String,
        reach: TagStreamReach,
        hot_tags_input: &HotTagsInput,
    ) -> Result<Option<HotTags>, DynError> {
        let query = queries::get::get_hot_tags_by_reach(user_id.as_str(), reach, hot_tags_input);
        retrieve_from_graph::<HotTags>(query, "hot_tags").await
    }

    /// Retrieves global hot tags, checking the cache first before querying the database.
    /// This function first attempts to fetch global hot tags from the cache. If the cached
    /// data is unavailable, it queries the graph database to retrieve the latest hot tags.
    /// If new data is found, it updates the cache before returning the results.
    ///
    /// # Arguments
    ///
    /// * `hot_tags_input` - The input parameters received from the API endpoint
    async fn get_global_hot_tags(
        hot_tags_input: &HotTagsInput,
    ) -> Result<Option<HotTags>, DynError> {
        let cached_hot_tags = HotTags::get_from_global_cache(hot_tags_input).await?;
        // If it is cache miss, retry all the info related from the graph
        if cached_hot_tags.is_some() {
            return Ok(cached_hot_tags);
        }
        let hot_tag_input = HotTagsInput::new(
            hot_tags_input.timeframe.clone(),
            100,
            0,
            20,
            hot_tags_input.tagged_type.clone(),
        );
        let query = queries::get::get_global_hot_tags(&hot_tag_input);
        let result = retrieve_from_graph::<HotTags>(query, "hot_tags").await?;

        let hot_tags = match result {
            Some(hot_tags) => hot_tags,
            None => return Ok(None),
        };
        if hot_tags.len() > 0 {
            HotTags::set_to_global_cache(hot_tags.clone(), hot_tags_input).await?;
        }

        HotTags::get_from_global_cache(hot_tags_input).await
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
        hot_tags_input: &HotTagsInput,
    ) -> Result<Option<HotTags>, DynError> {
        let timeframe = hot_tags_input.timeframe.to_string();
        let (hot_tag_key_parts, taggers_key_parts) = Self::build_hot_tags_key_parts(&timeframe);

        let hot_tag_taggers = HotTagsTaggers::try_from_index_json(
            Some(HOT_TAGS_CACHE_PREFIX.to_string()),
            &taggers_key_parts,
        )
        .await?;

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
            (Some(score_list), Some(taggers)) => (score_list, taggers),
            _ => return Ok(None),
        };

        let mut hot_tags = Vec::with_capacity(hot_tags_score.len());

        for (label, score) in hot_tags_score {
            if let Some(tag) = hot_tag_taggers.get(&label) {
                // Reduce taggers list
                let taggers_id: Vec<String> = tag
                    .iter()
                    .take(hot_tags_input.taggers_limit)
                    .cloned()
                    .collect();
                hot_tags.push(HotTag {
                    label,
                    taggers_id: Taggers(taggers_id),
                    tagged_count: score as u64,
                    taggers_count: tag.len(),
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
        hot_tags_input: &HotTagsInput,
    ) -> Result<(), DynError> {
        let timeframe = hot_tags_input.timeframe.to_string();
        let (hot_tag_key_parts, taggers_key_parts) = Self::build_hot_tags_key_parts(&timeframe);

        let mut hot_tags_score = Vec::with_capacity(hot_tags_list.len());

        let taggers: HashMap<String, Taggers> = hot_tags_list
            .iter()
            .map(|tag| {
                hot_tags_score.push((tag.tagged_count as f64, tag.label.as_str()));
                (tag.label.clone(), tag.taggers_id.clone())
            })
            .collect();

        // Store the taggers as JSON in cache
        HotTagsTaggers(taggers)
            .put_index_json(
                Some(HOT_TAGS_CACHE_PREFIX.to_string()),
                taggers_key_parts.as_slice(),
                Some(hot_tags_input.timeframe.to_cache_period()),
            )
            .await?;

        // Store the score as sorted set in cache
        HotTags::put_index_sorted_set(
            &hot_tag_key_parts,
            &hot_tags_score,
            Some(HOT_TAGS_CACHE_PREFIX),
            Some(hot_tags_input.timeframe.to_cache_period()),
        )
        .await?;
        Ok(())
    }

    /// Builds key parts for hot tags and hot tag taggers based on the given timeframe
    ///
    /// # Arguments
    /// * `timeframe` - A string slice representing the timeframe (e.g., "today", "this_month", "all_time").
    fn build_hot_tags_key_parts(timeframe: &str) -> (Vec<&str>, Vec<&str>) {
        let hot_tag_key_parts = [&POST_HOT_TAGS[..], &[timeframe]].concat();
        let taggers_key_parts = [&POST_HOT_TAGS[..], &[TAGGERS], &[timeframe]].concat();

        (hot_tag_key_parts, taggers_key_parts)
    }

    /// Reindexes global hot tags
    /// Retrieves and updates global hot tags for different timeframes. It fetches the top 100 hot tags
    ///  with a taggers limit of 20 for both "all-time" and "this month" timeframes
    pub async fn reindex() -> Result<(), DynError> {
        let all_timeframe_input =
            HotTagsInput::new(Timeframe::AllTime, 100, 0, 20, Some(TaggedType::Post));
        HotTags::get_global_hot_tags(&all_timeframe_input).await?;

        let month_timeframe_input =
            HotTagsInput::new(Timeframe::ThisMonth, 100, 0, 20, Some(TaggedType::Post));
        HotTags::get_global_hot_tags(&month_timeframe_input).await?;
        Ok(())
    }
}
