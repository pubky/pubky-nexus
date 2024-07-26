use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use crate::db::connectors::neo4j::get_neo4j_graph;
use super::Tags;
use crate::queries;

/// Represents a tag with its tag label, count, and author sources.
#[derive(Serialize, Deserialize, ToSchema, Debug)]
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
        match Self::get_from_index(user_id).await.unwrap() {
            Some(user_tags) => Ok(Some(user_tags)),
            None => Self::get_from_graph(user_id).await
        }
    }

    async fn get_from_index(user_id: &str) -> Result<Option<Vec<UserTag>>, Box<dyn std::error::Error + Send + Sync>> {
        let search_keys = Tags::search_keys_with_pattern(user_id).await?;

        let mut user_tags: Vec<UserTag> = vec![];

        for key in search_keys {
            //Get the value associated with the key
            let by = Tags::search_key_value(&key).await?;
            let label_str = key.as_str().rsplit(':').next().unwrap_or("");
            // Populate the vector
            user_tags.push(UserTag { label: String::from(label_str), by});
        }

        // TODO#35: Treat in anohter way the result if it is possible
        if !user_tags.is_empty() {
            return Ok(Some(user_tags))
        }

        Ok(None)
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
            for UserTag {label, by} in &tagged_from {
                let key = &format!("{:}:{:}", user_id, label);
                Tags::set_index(key, by).await?
            }
            return Ok(Some(tagged_from))
        }
        Ok(None)
    }
}




