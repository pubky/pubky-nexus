use crate::db::connectors::redis::get_redis_conn;
use redis::{AsyncCommands, AsyncIter, JsonAsyncCommands};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use crate::db::connectors::neo4j::get_neo4j_graph;
use super::Tag;
use crate::queries;

use crate::db::kv::index;

/// Represents a tag with its tag label, count, and author sources.
#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct UserTag {
    pub label: String,
    by: Vec<Tag>
}

impl Default for UserTag {
    fn default() -> Self {
        Self::new()
    }
}

impl UserTag {
    pub fn new() -> Self {
        Self {
            label: String::new(),
            by: Vec::new(),
        }
    }
}


// TODO#: Read if this is a way to do
#[derive(Serialize, Deserialize)]
pub struct UserTags {}

impl UserTags {

    pub async fn get_by_id(
        user_id: &str
    ) -> Result<Option<Vec<UserTag>>, Box<dyn std::error::Error + Send + Sync>> {
        match Self::get_from_index(user_id).await.unwrap() {
            Some(user_tags) => Ok(Some(user_tags)),
            None => Self::get_from_graph(user_id).await
        }
    }

    async fn get_from_index(user_id: &str) -> Result<Option<Vec<UserTag>>, Box<dyn std::error::Error + Send + Sync>> {
        let key_prefix = &Tag::type_name().await;
        let pattern = format!("{}{:}*", key_prefix, user_id);

        // TODO#: find some way to keep open the connection
        let mut redis_conn = get_redis_conn().await?;

        // Search base on regular expression
        let mut iter:AsyncIter<Option<String>> = redis_conn.scan_match(pattern).await.unwrap();

        let mut user_tags: Vec<UserTag> = vec![];

        while let Some(wrapped_key) = iter.next_item().await {
            if let Some(key) = wrapped_key {
                //Get the value associated with the key
                let mut redis_conn = get_redis_conn().await?;
                let value: String = redis_conn.json_get(&key, ".").await.unwrap();
                let label_str = key.as_str().rsplit(':').next().unwrap_or("");
                let by: Vec<Tag> = serde_json::from_str(&value)?;
                // Populate the vector
                user_tags.push(UserTag { label: String::from(label_str), by});
            }
        }

        // TODO#: Treat in anohter way the result if it is possible
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
                // TODO#: Try to use RedisOps, with struct Tags(Vec<Tag>)
                // For that Implement Deref so Tags can be used like Vec<Tag>
                index::set(
                    &Tag::type_name().await,
                    &format!("{:}:{:}", user_id, label),
                    by,
                    None, 
                    None)
                .await?; // Add a blank line between ProfileTags
            }
            return Ok(Some(tagged_from))
        }
        Ok(None)
    }
}




