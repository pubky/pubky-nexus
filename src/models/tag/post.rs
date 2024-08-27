use crate::db::kv::index::sorted_sets::Sorting;
use crate::{db::connectors::neo4j::get_neo4j_graph, RedisOps};
use crate::queries;
use axum::async_trait;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use utoipa::ToSchema;
use std::error::Error;

use super::TagDetails;

const POST_TAGS_KEY_PARTS: [&str; 2] = ["Posts", "Tag"];

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema, Default)]
pub struct TagPost(Vec<TagDetails>);

// Implement Deref so TagList can be used like Vec<String>
impl Deref for TagPost {
    type Target = Vec<TagDetails>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
#[async_trait]
impl RedisOps for TagPost {
    async fn prefix() -> String {
        String::from("Post:Taggers")
    }
}

impl TagPost {

    fn create_set_key_parts<'a>(user_id: &'a str, post_id: &'a str) -> Vec<&'a str> {
        [&POST_TAGS_KEY_PARTS[..], &[user_id, post_id]].concat()
    }

    pub async fn get_by_id(
        user_id: &str,
        post_id: &str,
        max_tags: Option<usize>,
        max_taggers: Option<usize>
    ) -> Result<Option<TagPost>, Box<dyn std::error::Error + Send + Sync>> {
        // TODO: Not sure if this is the place to do or in the endpoint
        let max_tags = max_tags.unwrap_or(5);
        let max_taggers = max_taggers.unwrap_or(5);
        match Self::try_from_cache(user_id, post_id, max_tags, max_taggers).await? {
            Some(counts) => Ok(Some(counts)),
            None => Self::get_from_graph(user_id, post_id).await,
        }
    }

    async fn try_from_cache(user_id: &str, post_id: &str, max_tags: usize, max_taggers: usize
    ) -> Result<Option<TagPost>, Box<dyn std::error::Error + Send + Sync>> {
        let key_parts = Self::create_set_key_parts(user_id, post_id);
        match Self::try_from_index_sorted_set(&key_parts, None, None, None, Some(max_tags), Sorting::Descending).await? {
            Some(tag_scores) => {
                let mut tags = Vec::with_capacity(max_tags);
                for (label, _) in tag_scores.iter() {
                    tags.push(format!("{}:{}:{}", user_id, post_id, label));
                }
                let tags_ref: Vec<&str> = tags.iter().map(|label| label.as_str()).collect();
                let taggers = Self::try_from_multiple_sets(&tags_ref, Some(max_taggers)).await?;
                let tag_details_list = TagDetails::from_cache(tag_scores, taggers);
                Ok(Some(TagPost(tag_details_list)))
            },
            None => Ok(None)
        }
    }

    pub async fn get_from_graph(
        user_id: &str,
        post_id: &str,
    ) -> Result<Option<TagPost>, Box<dyn std::error::Error + Send + Sync>> {
        let mut result;
        {
            // We cannot use LIMIT clause because we need all data related
            let query = queries::post_tags(user_id, post_id);
            let graph = get_neo4j_graph()?;

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        if let Some(row) = result.next().await? {
            let user_exists: bool = row.get("post_exists").unwrap_or(false);
            if user_exists {
                let tagged_from: TagPost = row.get("post_tags").unwrap_or_default();
                Self::add_to_label_sorted_set(user_id, post_id, &tagged_from).await?;
                return Ok(Some(tagged_from));
            }
        }
        Ok(None)
    }

    async fn add_to_label_sorted_set(user_id: &str, post_id: &str, tags: &[TagDetails]) -> Result<(), Box<dyn Error + Send + Sync>> {
        let (tag_scores, (labels, taggers)) = TagDetails::split_fields_and_calculate_scores(tags);

        let key_parts = Self::create_set_key_parts(user_id, post_id);
        Self::put_index_sorted_set(&key_parts, tag_scores.as_slice()).await?;

        Self::put_multiple_set_indexes(&[user_id, post_id], &labels, &taggers).await
    }
}
