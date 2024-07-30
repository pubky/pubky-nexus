use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use crate::db::connectors::neo4j::get_neo4j_graph;
use super::Tags;
use crate::queries;

/// Represents a tag that refers to the current user
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub struct UserTag {
    pub label: String,
    by: Tags
}

impl Default for UserTag {
    fn default() -> Self {
        Self {
            label: String::new(),
            by: Tags::default(),
        }
    }
}


// TODO#35: Read if this is a way to do
#[derive(Serialize, Deserialize)]
pub struct UserTags {}

impl UserTags {

    pub async fn get_by_id(user_id: &str) -> Result<Option<Vec<UserTag>>, Box<dyn std::error::Error + Send + Sync>> {
        Self::get_from_graph(user_id).await
    }

    async fn get_from_graph(
        user_id: &str
    ) -> Result<Option<Vec<UserTag>>, Box<dyn std::error::Error + Send + Sync>> {
        let query = queries::user_tags(user_id);
        let graph = get_neo4j_graph()?;

        let graph = graph.lock().await;
        let mut result = graph.execute(query).await?;

        if let Some(row) = result.next().await? {
            // Deserialize query value to Vec
            let tagged_from: Vec<UserTag> = row.get("user_tags").unwrap();
            return Ok(Some(tagged_from))
        }
        Ok(None)
    }
}
