use axum::async_trait;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::ops::Deref;
use utoipa::ToSchema;

use crate::db::graph::exec::retrieve_from_graph;
use crate::db::kv::index::sorted_sets::Sorting;
use crate::models::user::UserStreamType;
use crate::{db::connectors::neo4j::get_neo4j_graph, queries};
use crate::{RedisOps, ScoreAction};

pub const TAG_GLOBAL_HOT: [&str; 3] = ["Tags", "Global", "Hot"];

#[derive(Serialize, Deserialize, Debug, ToSchema)]
pub struct Taggers(Vec<String>);

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
    fn from_vec(vec: Vec<String>) -> Self {
        Self(vec)
    }

    pub async fn update_index_score(
        label: &str,
        score_action: ScoreAction,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Self::put_score_index_sorted_set(&TAG_GLOBAL_HOT, &[label], score_action).await
    }

    pub async fn put_to_index(
        label: &str,
        user_id: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Self::put_index_set(&[label], &[user_id]).await
    }
}

#[derive(Deserialize, Serialize, ToSchema, Debug)]
pub struct HotTag {
    label: String,
    taggers_id: Taggers,
    tagged_count: u64,
    taggers_count: usize,
}

// Define a newtype wrapper
#[derive(Serialize, Deserialize, Debug, ToSchema, Default)]
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
    pub async fn set_global_tag_scores() -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let graph = graph.lock().await;

            let query = queries::read::get_global_hot_tags_scores();
            result = graph.execute(query).await?;
        }

        if let Some(row) = result.next().await? {
            let hot_tags_score: Vec<(f64, &str)> = row.get("hot_tags_score").unwrap_or(Vec::new());
            let hot_tags_users: Vec<(&str, Vec<String>)> =
                row.get("hot_tags_users").unwrap_or(Vec::new());
            // Make sure both list has content before write the indexes
            if !hot_tags_score.is_empty() && !hot_tags_users.is_empty() {
                Self::put_index_sorted_set(&TAG_GLOBAL_HOT, hot_tags_score.as_slice()).await?;
                // Add all the users_id in the SET
                for (label, user_list) in hot_tags_users.into_iter() {
                    let values_ref: Vec<&str> = user_list.iter().map(|id| id.as_str()).collect();
                    Taggers::put_index_set(&[label], &values_ref).await?;
                }
            }
        }
        Ok(())
    }

    pub async fn get_global_tags_stream(
        skip: Option<usize>,
        limit: Option<usize>,
        taggers_limit: Option<usize>,
    ) -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
        let hot_tags = Self::try_from_index_sorted_set(
            &TAG_GLOBAL_HOT,
            None,
            None,
            skip,
            limit,
            Sorting::Descending,
        )
        .await?
        .unwrap_or_default();

        if hot_tags.is_empty() {
            return Ok(None);
        }

        // Collect the labels as a vector of string slices
        let labels: Vec<&str> = hot_tags.iter().map(|(label, _)| label.as_str()).collect();
        let label_slice: &[&str] = &labels;

        let list = Taggers::try_from_multiple_sets(label_slice, taggers_limit).await?;

        let hot_tags_stream: HotTags = hot_tags
            .into_iter()
            .zip(list)
            .filter_map(|((label, score), user_ids)| match user_ids {
                Some((tagger_list, taggers_count)) => {
                    let taggers_id = Taggers::from_vec(tagger_list);
                    Some(HotTag {
                        label,
                        taggers_id,
                        tagged_count: score as u64,
                        taggers_count,
                    })
                }
                None => None,
            })
            .collect();
        Ok(Some(hot_tags_stream))
    }

    pub async fn get_hot_tags_by_reach(
        user_id: String,
        reach: UserStreamType,
        skip: usize,
        limit: usize,
        max_taggers: usize,
    ) -> Result<Option<HotTags>, Box<dyn Error + Send + Sync>> {
        let query = queries::read::get_hot_tags_by_reach(
            &user_id,
            reach.to_graph_subquery(),
            skip,
            limit,
            max_taggers,
        );
        retrieve_from_graph::<HotTags>(query, "hot_tags").await
    }
}
