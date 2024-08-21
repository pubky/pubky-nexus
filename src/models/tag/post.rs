use std::ops::Deref;

use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::queries;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::Tags;

/// Represents a tag that refers to the current user
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone, Default)]
pub struct PostTag {
    pub label: String,
    tagged: Tags,
}

// Define a newtype wrapper
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema, Default)]
pub struct PostTags(Vec<PostTag>);

// Implement Deref so TagList can be used like Vec<String>
impl Deref for PostTags {
    type Target = Vec<PostTag>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PostTags {
    pub async fn get_by_id(
        user_id: &str,
        post_id: &str,
    ) -> Result<Option<PostTags>, Box<dyn std::error::Error + Send + Sync>> {
        Self::get_from_graph(user_id, post_id).await
    }

    async fn get_from_graph(
        user_id: &str,
        post_id: &str,
    ) -> Result<Option<PostTags>, Box<dyn std::error::Error + Send + Sync>> {
        let mut result;
        {
            let query = queries::post_tags(user_id, post_id);
            let graph = get_neo4j_graph()?;

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        if let Some(row) = result.next().await? {
            let user_exists: bool = row.get("post_exists").unwrap_or(false);
            if user_exists {
                let tagged_from: PostTags = row.get("post_tags").unwrap_or_default();
                return Ok(Some(tagged_from));
            }
        }
        Ok(None)
    }
}
