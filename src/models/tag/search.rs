use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::db::kv::index::sorted_sets::Sorting;
use crate::models::post::PostStream;
use crate::queries::read::{global_tags_by_post, global_tags_by_post_engagement};
use crate::RedisOps;
use neo4rs::Query;
use serde::{Deserialize, Serialize};
use std::error::Error;
use utoipa::ToSchema;

pub const TAG_GLOBAL_POST_TIMELINE: [&str; 4] = ["Tags", "Global", "Post", "Timeline"];
pub const TAG_GLOBAL_POST_ENGAGEMENT: [&str; 4] = ["Tags", "Global", "Post", "Engagement"];

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SortBy {
    Timeline,
    Engagement,
}

#[derive(Serialize, Deserialize, ToSchema, Default)]
pub struct TagSearch {}

impl RedisOps for TagSearch {}

impl TagSearch {
    /// Indexes post tags into global sorted sets for timeline and engagement metrics.
    pub async fn index_post_tags_from_graph() -> Result<(), Box<dyn Error + Send + Sync>> {
        Self::add_to_global_sorted_set(global_tags_by_post(), TAG_GLOBAL_POST_TIMELINE).await?;
        Self::add_to_global_sorted_set(
            global_tags_by_post_engagement(),
            TAG_GLOBAL_POST_ENGAGEMENT,
        )
        .await?;
        Ok(())
    }

    pub async fn get_by_label(
        label: &str,
        sort_by: Option<SortBy>,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<PostStream>, Box<dyn Error + Send + Sync>> {
        let skip = skip.unwrap_or(0);
        let limit = limit.unwrap_or(6);

        let posts_sorted_set = match sort_by {
            Some(SortBy::Engagement) => {
                Self::try_from_index_sorted_set(
                    &[&TAG_GLOBAL_POST_ENGAGEMENT[..], &[label]].concat(),
                    None,
                    None,
                    Some(skip),
                    Some(limit),
                    Sorting::Descending,
                )
                .await?
            }
            // Default case always: SortBy::Timeline
            _ => {
                Self::try_from_index_sorted_set(
                    &[&TAG_GLOBAL_POST_TIMELINE[..], &[label]].concat(),
                    None,
                    None,
                    Some(skip),
                    Some(limit),
                    Sorting::Descending,
                )
                .await?
            }
        };

        match posts_sorted_set {
            Some(post_keys) => {
                let post_keys: Vec<String> = post_keys.into_iter().map(|(key, _)| key).collect();
                PostStream::from_listed_post_ids(None, &post_keys).await
            }
            None => Ok(None),
        }
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
            let label: &str = row.get("label")?;
            let sorted_set: Vec<(f64, &str)> = row.get("sorted_set")?;
            let key_parts = [&index_key[..], &[label]].concat();
            Self::put_index_sorted_set(&key_parts, &sorted_set).await?;
        }
        Ok(())
    }
}
