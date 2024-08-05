use std::ops::Deref;

use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::queries;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::Tags;

/// Represents a tag that refers to the current user
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
#[derive(Default)]
pub struct UserTag {
    pub label: String,
    tagged: Tags,
}


// Define a newtype wrapper
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[derive(Default)]
pub struct UserTags(Vec<UserTag>);


// Implement Deref so TagList can be used like Vec<String>
impl Deref for UserTags {
    type Target = Vec<UserTag>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl UserTags {
    pub async fn get_by_id(
        user_id: &str,
    ) -> Result<Option<UserTags>, Box<dyn std::error::Error + Send + Sync>> {
        Self::get_from_graph(user_id).await
    }

    async fn get_from_graph(
        user_id: &str,
    ) -> Result<Option<UserTags>, Box<dyn std::error::Error + Send + Sync>> {
        let query = queries::user_tags(user_id);
        let graph = get_neo4j_graph()?;

        let graph = graph.lock().await;
        let mut result = graph.execute(query).await?;

        if let Some(row) = result.next().await? {
            let user_exists: bool = row.get("user_exists").unwrap_or(false);
            if user_exists {
                let tagged_from: UserTags = row.get("user_tags").unwrap_or_default();
                return Ok(Some(tagged_from));
            }
        }
        Ok(None)
    }
}
