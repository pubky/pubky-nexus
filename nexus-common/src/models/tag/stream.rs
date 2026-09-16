use crate::db::kv::{RedisResult, SortOrder};
use crate::db::{fetch_key_from_graph, queries, RedisOps};
use crate::models::error::ModelResult;
use crate::types::routes::HotTagsInputDTO;
use crate::types::{StreamReach, Timeframe};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Deref;
use tracing::{debug, warn};
use utoipa::ToSchema;

use super::global::{HotTagsTaggers, Taggers};
use super::TaggedType;

pub const HOT_TAGS_CACHE_PREFIX: &str = "Cache";
pub const POST_HOT_TAGS: [&str; 3] = ["Tags", "Post", "Hot"];
/// Snapshot size per timeframe. A skip past this cannot be filled from cache.
pub const GLOBAL_HOT_TAGS_CACHE_SIZE: usize = 100;
const GLOBAL_HOT_TAGS_TAGGERS_LIMIT: usize = 20;

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

    /// Cache hit, including an empty page. On a missing key, refresh then re-read
    /// so `skip`/`limit`/`taggers_limit` apply to the snapshot, not the graph result.
    async fn get_global_hot_tags(hot_tags_input: &HotTagsInputDTO) -> ModelResult<Option<HotTags>> {
        if let Some(cached) = HotTags::get_from_global_cache(hot_tags_input).await? {
            return Ok(Some(cached));
        }

        // A skip past the snapshot is an empty page no matter what the graph holds,
        // so refreshing for it would only let a caller walk `skip` to force scans.
        if hot_tags_input.skip >= GLOBAL_HOT_TAGS_CACHE_SIZE {
            return Ok(Some(HotTags::default()));
        }

        HotTags::fetch_and_cache(&hot_tags_input.timeframe).await?;
        HotTags::get_from_global_cache(hot_tags_input)
            .await
            .map_err(Into::into)
    }

    /// Scan the top 100 post tags and replace the cache. Empty/`None` leaves the
    /// previous ranking in place.
    pub async fn fetch_and_cache(timeframe: &Timeframe) -> ModelResult<()> {
        let query_input = HotTagsInputDTO::new(
            timeframe.clone(),
            GLOBAL_HOT_TAGS_CACHE_SIZE,
            0,
            GLOBAL_HOT_TAGS_TAGGERS_LIMIT,
            Some(TaggedType::Post),
        );
        let query = queries::get::get_global_hot_tags(&query_input);
        let result = fetch_key_from_graph::<HotTags>(query, "hot_tags").await?;
        HotTags::write_or_preserve_cache(result, timeframe, HOT_TAGS_CACHE_PREFIX).await
    }

    /// Non-empty result replaces both keys. Empty/`None` is a no-op.
    /// Tests pass their own `prefix` so they never touch production keys.
    async fn write_or_preserve_cache(
        result: Option<HotTags>,
        timeframe: &Timeframe,
        prefix: &str,
    ) -> ModelResult<()> {
        match result {
            Some(hot_tags) if !hot_tags.is_empty() => {
                debug!(%timeframe, count = hot_tags.len(), "Writing hot tags cache");
                HotTags::put_to_global_cache(hot_tags, timeframe, prefix).await?;
            }
            Some(_) => {
                warn!(%timeframe, "Graph returned empty hot tags — previous cache left untouched");
            }
            None => {
                warn!(%timeframe, "Graph returned no hot tags — previous cache left untouched");
            }
        }
        Ok(())
    }

    /// `None` if either key is missing. `Some([])` if both exist but this window is empty.
    async fn get_from_global_cache(
        hot_tags_input: &HotTagsInputDTO,
    ) -> RedisResult<Option<HotTags>> {
        let timeframe = hot_tags_input.timeframe.to_string();
        let key_parts = Self::build_hot_tags_key_parts(&timeframe);

        let taggers_by_label = Taggers::get_from_index(&timeframe, HOT_TAGS_CACHE_PREFIX).await?;
        let scores = HotTags::try_from_index_sorted_set(
            &key_parts,
            None,
            None,
            Some(hot_tags_input.skip),
            Some(hot_tags_input.limit),
            SortOrder::Descending,
            Some(HOT_TAGS_CACHE_PREFIX),
        )
        .await?;

        let (Some(scores), Some(taggers_by_label)) = (scores, taggers_by_label) else {
            return Ok(None);
        };
        if scores.is_empty() {
            return Ok(Some(HotTags::default()));
        }

        let hot_tags = scores
            .into_iter()
            .filter_map(|(label, score)| {
                let taggers = taggers_by_label.get(&label)?;
                Some(HotTag {
                    label,
                    taggers_id: Taggers(Taggers::get_taggers_by_pagination(
                        taggers,
                        0,
                        hot_tags_input.taggers_limit,
                    )),
                    tagged_count: score as u64,
                    taggers_count: taggers.len(),
                })
            })
            .collect();
        Ok(Some(hot_tags))
    }

    /// Overwrite taggers JSON and atomically replace the score set.
    async fn put_to_global_cache(
        hot_tags_list: HotTags,
        timeframe: &Timeframe,
        prefix: &str,
    ) -> ModelResult<()> {
        let timeframe_str = timeframe.to_string();
        let key_parts = Self::build_hot_tags_key_parts(&timeframe_str);
        let scores: Vec<(f64, &str)> = hot_tags_list
            .iter()
            .map(|tag| (tag.tagged_count as f64, tag.label.as_str()))
            .collect();
        let taggers: HashMap<String, Taggers> = hot_tags_list
            .iter()
            .map(|tag| (tag.label.clone(), tag.taggers_id.clone()))
            .collect();

        Taggers::put_to_index(HotTagsTaggers(taggers), timeframe, prefix).await?;
        HotTags::replace_index_sorted_set(
            &key_parts,
            &scores,
            Some(prefix),
            Some(timeframe.to_cache_period()),
        )
        .await?;
        Ok(())
    }

    fn build_hot_tags_key_parts(timeframe: &str) -> Vec<&str> {
        [&POST_HOT_TAGS[..], &[timeframe]].concat()
    }

    /// Warm AllTime and ThisMonth from the graph.
    pub async fn reindex() -> ModelResult<()> {
        HotTags::fetch_and_cache(&Timeframe::AllTime).await?;
        HotTags::fetch_and_cache(&Timeframe::ThisMonth).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{types::DynError, StackConfig, StackManager};

    /// Off the production `HOT_TAGS_CACHE_PREFIX` keys the API tests share.
    const TEST_PREFIX: &str = "HotTagsCacheTest";

    #[tokio_shared_rt::test(shared)]
    async fn write_or_preserve_cache_keeps_existing_ranking_on_empty_graph_result(
    ) -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let timeframe = Timeframe::Today;
        HotTags::put_to_global_cache(
            HotTags(vec![
                hot_tag("bitcoin", 10, &["alice"]),
                hot_tag("nostr", 5, &["bob"]),
            ]),
            &timeframe,
            TEST_PREFIX,
        )
        .await?;

        HotTags::write_or_preserve_cache(Some(HotTags::default()), &timeframe, TEST_PREFIX).await?;
        assert_cached_labels(&timeframe, &["bitcoin", "nostr"]).await?;

        clear_test_cache(&timeframe).await?;
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn write_or_preserve_cache_keeps_existing_ranking_on_none_graph_result(
    ) -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let timeframe = Timeframe::ThisWeek;
        HotTags::put_to_global_cache(
            HotTags(vec![hot_tag("pubky", 20, &["carol"])]),
            &timeframe,
            TEST_PREFIX,
        )
        .await?;

        HotTags::write_or_preserve_cache(None, &timeframe, TEST_PREFIX).await?;
        assert_cached_labels(&timeframe, &["pubky"]).await?;

        clear_test_cache(&timeframe).await?;
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn write_or_preserve_cache_replaces_existing_ranking_on_non_empty_graph_result(
    ) -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let timeframe = Timeframe::ThisMonth;
        HotTags::put_to_global_cache(
            HotTags(vec![
                hot_tag("stale", 1, &["dave"]),
                hot_tag("dropped", 2, &["erin"]),
            ]),
            &timeframe,
            TEST_PREFIX,
        )
        .await?;

        HotTags::write_or_preserve_cache(
            Some(HotTags(vec![hot_tag("fresh", 99, &["frank"])])),
            &timeframe,
            TEST_PREFIX,
        )
        .await?;
        assert_cached_labels(&timeframe, &["fresh"]).await?;

        clear_test_cache(&timeframe).await?;
        Ok(())
    }

    fn hot_tag(label: &str, tagged_count: u64, taggers: &[&str]) -> HotTag {
        HotTag {
            label: label.to_string(),
            taggers_id: Taggers(taggers.iter().map(|id| id.to_string()).collect()),
            tagged_count,
            taggers_count: taggers.len(),
        }
    }

    async fn assert_cached_labels(
        timeframe: &Timeframe,
        expected: &[&str],
    ) -> Result<(), DynError> {
        let scores = read_raw_scores(timeframe)
            .await?
            .expect("cache scores must exist");
        let taggers = read_raw_taggers(timeframe)
            .await?
            .expect("cache taggers must exist");
        assert_eq!(scores.len(), expected.len());
        assert_eq!(taggers.len(), expected.len());
        for label in expected {
            assert!(
                scores.iter().any(|(cached, _)| cached == label),
                "missing {label} in scores"
            );
            assert!(taggers.get(*label).is_some(), "missing {label} in taggers");
        }
        Ok(())
    }

    async fn read_raw_scores(timeframe: &Timeframe) -> RedisResult<Option<Vec<(String, f64)>>> {
        let timeframe_str = timeframe.to_string();
        let key_parts = HotTags::build_hot_tags_key_parts(&timeframe_str);
        HotTags::try_from_index_sorted_set(
            &key_parts,
            None,
            None,
            Some(0),
            Some(100),
            SortOrder::Descending,
            Some(TEST_PREFIX),
        )
        .await
    }

    async fn read_raw_taggers(timeframe: &Timeframe) -> RedisResult<Option<HotTagsTaggers>> {
        Taggers::get_from_index(&timeframe.to_string(), TEST_PREFIX).await
    }

    async fn clear_test_cache(timeframe: &Timeframe) -> RedisResult<()> {
        let timeframe_str = timeframe.to_string();
        let key_parts = HotTags::build_hot_tags_key_parts(&timeframe_str);
        HotTags::replace_index_sorted_set(&key_parts, &[], Some(TEST_PREFIX), None).await?;
        Taggers::put_to_index(HotTagsTaggers(HashMap::new()), timeframe, TEST_PREFIX).await
    }
}
