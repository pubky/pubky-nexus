use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::{queries, RedisOps};
use chrono::Utc;
use neo4rs::Relation;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::PostStream;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct Bookmark {
    id: String,
    pub indexed_at: i64,
}

impl RedisOps for Bookmark {}

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

    /// Retrieves counts by user ID, first trying to get from Redis, then from Neo4j if not found.
    pub async fn get_by_id(
        author_id: &str,
        post_id: &str,
        viewer_id: Option<&str>,
    ) -> Result<Option<Bookmark>, Box<dyn std::error::Error + Send + Sync>> {
        // Return None early if no viewer_id supplied
        let viewer_id = match viewer_id {
            Some(viewer_id) => viewer_id,
            None => return Ok(None),
        };
        match Self::try_from_index_json(&[author_id, post_id, viewer_id]).await? {
            Some(counts) => Ok(Some(counts)),
            None => Self::get_from_graph(author_id, post_id, viewer_id).await,
        }
    }

    /// Retrieves a bookmark from Neo4j.
    pub async fn get_from_graph(
        author_id: &str,
        post_id: &str,
        viewer_id: &str,
    ) -> Result<Option<Bookmark>, Box<dyn std::error::Error + Send + Sync>> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query = queries::post_bookmark(author_id, post_id, viewer_id);

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        if let Some(row) = result.next().await? {
            // TODO, research why sometimes there is a result that is not a Relation here ?
            let relation: Relation = match row.get("b") {
                Ok(value) => value,
                Err(_) => return Ok(None),
            };
            let bookmark = Self {
                id: relation.get("id").unwrap_or_default(),
                indexed_at: relation.get("indexed_at").unwrap_or_default(),
            };
            bookmark
                .put_index_json(&[author_id, post_id, viewer_id])
                .await?;
            PostStream::add_to_bookmarks_sorted_set(&bookmark, viewer_id, post_id, author_id)
                .await?;
            Ok(Some(bookmark))
        } else {
            Ok(None)
        }
    }

    /// Retrieves all post_keys a user bookmarked from Neo4j
    pub async fn index_all_from_graph(
        user_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query = queries::user_bookmarks(user_id);

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        let mut bookmarked_post_keys = Vec::new();

        while let Some(row) = result.next().await? {
            if let Some(relation) = row.get::<Option<Relation>>("b")? {
                let bookmark = Bookmark {
                    id: relation.get("id").unwrap_or_default(),
                    indexed_at: relation.get("indexed_at").unwrap_or_default(),
                };
                let author_id = row.get("author_id")?;
                let post_id = row.get("post_id")?;
                PostStream::add_to_bookmarks_sorted_set(&bookmark, user_id, post_id, author_id)
                    .await?;
                bookmarked_post_keys.push(format!("{}:{}", author_id, post_id));
            }
        }
        Ok(())
    }
}
