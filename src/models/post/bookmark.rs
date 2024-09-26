use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::{queries, RedisOps};
use neo4rs::Relation;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::PostStream;

#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
pub struct Bookmark {
    pub id: String,
    pub indexed_at: i64,
}

impl RedisOps for Bookmark {}

impl Bookmark {
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
        match Self::get_from_index(author_id, post_id, viewer_id).await? {
            Some(counts) => Ok(Some(counts)),
            None => {
                let graph_response = Self::get_from_graph(author_id, post_id, viewer_id).await?;
                if let Some(bookmark) = graph_response {
                    bookmark.put_to_index(author_id, post_id, viewer_id).await?;
                    return Ok(Some(bookmark));
                }
                Ok(None)
            }
        }
    }

    pub async fn get_from_index(
        author_id: &str,
        post_id: &str,
        viewer_id: &str,
    ) -> Result<Option<Bookmark>, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(bookmark) = Self::try_from_index_json(&[author_id, post_id, viewer_id]).await? {
            return Ok(Some(bookmark));
        }
        Ok(None)
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
            let query = queries::read::post_bookmark(author_id, post_id, viewer_id);

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
            Ok(Some(bookmark))
        } else {
            Ok(None)
        }
    }

    pub async fn put_to_index(
        &self,
        author_id: &str,
        post_id: &str,
        viewer_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.put_index_json(&[author_id, post_id, viewer_id])
            .await?;
        PostStream::add_to_bookmarks_sorted_set(self, viewer_id, post_id, author_id).await?;
        Ok(())
    }

    /// Retrieves all post_keys a user bookmarked from Neo4j
    /// TODO: using in reindex, Refactor
    pub async fn reindex(user_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query = queries::read::user_bookmarks(user_id);

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        while let Some(row) = result.next().await? {
            if let Some(relation) = row.get::<Option<Relation>>("b")? {
                let bookmark = Bookmark {
                    id: relation.get("id").unwrap_or_default(),
                    indexed_at: relation.get("indexed_at").unwrap_or_default(),
                };
                let author_id = row.get("author_id")?;
                let post_id = row.get("post_id")?;
                bookmark.put_to_index(author_id, post_id, user_id).await?;
            }
        }
        Ok(())
    }
}
