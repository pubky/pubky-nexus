use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::db::kv::index::sorted_sets::Sorting;
use crate::models::post::PostDetails;
use crate::models::tag::traits::TaggersCollection;
use crate::queries::get::{global_tags_by_post, global_tags_by_post_engagement};
use crate::routes::v0::queries::PaginationQuery;
use crate::routes::v0::stream::PostStreamSorting;
use crate::{RedisOps, ScoreAction};
use neo4rs::Query;
use serde::{Deserialize, Serialize};
use std::error::Error;
use utoipa::ToSchema;

use super::post::TagPost;

pub const TAG_GLOBAL_POST_TIMELINE: [&str; 4] = ["Tags", "Global", "Post", "Timeline"];
pub const TAG_GLOBAL_POST_ENGAGEMENT: [&str; 4] = ["Tags", "Global", "Post", "TotalEngagement"];

/// Represents a single search result of post keys (`author_id:post_id`) by tags
#[derive(Serialize, Deserialize, ToSchema, Default)]
pub struct TagSearch {
    pub post_key: String,
    pub score: usize,
}

impl From<(String, f64)> for TagSearch {
    fn from(tuple: (String, f64)) -> Self {
        TagSearch {
            post_key: tuple.0,
            score: tuple.1 as usize,
        }
    }
}

impl RedisOps for TagSearch {}

impl TagSearch {
    /// Indexes post tags into global sorted sets for timeline and engagement metrics.
    pub async fn reindex() -> Result<(), Box<dyn Error + Send + Sync>> {
        Self::add_to_global_sorted_set(global_tags_by_post(), TAG_GLOBAL_POST_TIMELINE).await?;
        Self::add_to_global_sorted_set(
            global_tags_by_post_engagement(),
            TAG_GLOBAL_POST_ENGAGEMENT,
        )
        .await?;
        Ok(())
    }

    /// Retrieves post tags from a Neo4j graph and updates global sorted sets
    /// for both timeline and engagement-based metrics.
    async fn add_to_global_sorted_set(
        query: Query,
        index_key: [&str; 4],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        while let Some(row) = result.next().await? {
            let label: &str = row.get("label").unwrap_or("");
            let sorted_set: Vec<(f64, &str)> = row.get("sorted_set").unwrap_or(Vec::new());
            if !label.is_empty() && !sorted_set.is_empty() {
                let key_parts = [&index_key[..], &[label]].concat();
                Self::put_index_sorted_set(&key_parts, &sorted_set).await?;
            }
        }
        Ok(())
    }

    pub async fn get_by_label(
        label: &str,
        sort_by: Option<PostStreamSorting>,
        pagination: PaginationQuery,
    ) -> Result<Option<Vec<TagSearch>>, Box<dyn Error + Send + Sync>> {
        let post_score_list = match sort_by {
            Some(PostStreamSorting::TotalEngagement) => {
                Self::try_from_index_sorted_set(
                    &[&TAG_GLOBAL_POST_ENGAGEMENT[..], &[label]].concat(),
                    pagination.start,
                    pagination.end,
                    pagination.skip,
                    pagination.limit,
                    Sorting::Descending,
                )
                .await?
            }
            // Default case always: SortBy::Timeline
            _ => {
                Self::try_from_index_sorted_set(
                    &[&TAG_GLOBAL_POST_TIMELINE[..], &[label]].concat(),
                    pagination.start,
                    pagination.end,
                    pagination.skip,
                    pagination.limit,
                    Sorting::Descending,
                )
                .await?
            }
        };

        match post_score_list {
            Some(list) => Ok(Some(list.into_iter().map(|t| t.into()).collect())),
            None => Ok(None),
        }
    }

    pub async fn update_index_score(
        author_id: &str,
        post_id: &str,
        label: &str,
        score_action: ScoreAction,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let tag_global_engagement_key_parts = [&TAG_GLOBAL_POST_ENGAGEMENT[..], &[label]].concat();
        let post_key_slice: &[&str] = &[author_id, post_id];
        Self::put_score_index_sorted_set(
            &tag_global_engagement_key_parts,
            post_key_slice,
            score_action,
        )
        .await
    }

    pub async fn put_to_index(
        author_id: &str,
        post_id: &str,
        tag_label: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let post_key_slice: &[&str] = &[author_id, post_id];
        let key_parts = [&TAG_GLOBAL_POST_TIMELINE[..], &[tag_label]].concat();
        let tag_search = Self::check_sorted_set_member(&key_parts, post_key_slice)
            .await
            .unwrap();
        if tag_search.is_none() {
            let option = PostDetails::try_from_index_json(post_key_slice).await?;
            if let Some(post_details) = option {
                let member_key = post_key_slice.join(":");
                Self::put_index_sorted_set(
                    &key_parts,
                    &[(post_details.indexed_at as f64, &member_key)],
                )
                .await?;
            }
        }
        Ok(())
    }

    pub async fn del_from_index(
        author_id: &str,
        post_id: &str,
        tag_label: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let post_label_key = vec![author_id, post_id, tag_label];
        let label_taggers = TagPost::get_from_index(post_label_key, None, None).await?;
        // Make sure that post does not have more taggers with that tag. Post:Taggers:user_id:post_id:label
        if label_taggers.is_none() {
            let key_parts = [&TAG_GLOBAL_POST_TIMELINE[..], &[tag_label]].concat();
            let post_key = format!("{}:{}", author_id, post_id);
            Self::remove_from_index_sorted_set(&key_parts, &[&post_key]).await?;
        }
        Ok(())
    }
}
