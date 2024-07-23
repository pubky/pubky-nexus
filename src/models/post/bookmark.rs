use crate::models::Prefix;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct Bookmark {
    id: String,
    indexed_at: i64,
}

impl Default for Bookmark {
    fn default() -> Self {
        Self::new()
    }
}

impl Bookmark {
    pub fn new() -> Self {
        Self {
            id: String::new(),
            indexed_at: Utc::now().timestamp(),
        }
    }
    // /// Retrieves counts by user ID, first trying to get from Redis, then from Neo4j if not found.
    // pub async fn get_by_id(
    //     author_id: &str,
    //     post_id: &str,
    //     viewer_id: Option<&str>,
    // ) -> Result<Option<Bookmark>, Box<dyn std::error::Error + Send + Sync>> {
    //     match Bookmark::get_from_index(user_id).await? {
    //         Some(counts) => Ok(Some(counts)),
    //         None => Bookmark::get_from_graph(user_id).await,
    //     }
    // }

    // /// Sets counts in the Redis cache.
    // pub async fn set_index(
    //     &self,
    //     user_id: &str,
    // ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    //     index::set(&Self::prefix(), user_id, self, None, None).await
    // }

    // /// Get counts from the Redis cache.
    // pub async fn get_from_index(
    //     user_id: &str,
    // ) -> Result<Option<Self>, Box<dyn std::error::Error + Send + Sync>> {
    //     index::get(&Self::prefix(), user_id, None).await
    // }

    // /// Retrieves the counts from Neo4j.
    // pub async fn get_from_graph(
    //     user_id: &str,
    // ) -> Result<Option<Bookmark>, Box<dyn std::error::Error + Send + Sync>> {
    //     let graph = get_neo4j_graph()?;
    //     let query = queries::profile_counts(user_id);

    //     let graph = graph.lock().await;
    //     let mut result = graph.execute(query).await?;

    //     if let Some(row) = result.next().await? {
    //         if !row.get("user_exists").unwrap_or(false) {
    //             return Ok(None);
    //         }
    //         let counts = Bookmark {
    //             id: row.get("id").unwrap_or_default(),
    //             timestamp: row.get("timestamp").unwrap_or_default(),
    //         };
    //         counts.set_index(user_id).await?;
    //         Ok(Some(counts))
    //     } else {
    //         Ok(None)
    //     }
    // }
}
