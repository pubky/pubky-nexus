use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::models::Prefix;
use crate::{index, queries};
use chrono::Utc;
use neo4rs::Node;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents post data with content, bio, image, links, and status.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct PostDetails {
    content: String,
    id: String, // TODO: create Crockfordbase32 validator
    indexed_at: i64,
    author: String,
    uri: String,
}

impl Default for PostDetails {
    fn default() -> Self {
        Self::new()
    }
}

impl PostDetails {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            id: String::new(),
            indexed_at: Utc::now().timestamp(),
            author: String::new(),
            uri: String::new(),
        }
    }

    /// Retrieves post details by author ID and post ID, first trying to get from Redis, then from Neo4j if not found.
    pub async fn get_by_id(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<PostDetails>, Box<dyn std::error::Error + Send + Sync>> {
        match Self::get_from_index(author_id, post_id).await? {
            Some(details) => Ok(Some(details)),
            None => Self::get_from_graph(author_id, post_id).await,
        }
    }

    async fn from_node(node: &Node, author_id: &str) -> Self {
        let id = node.get("id").unwrap_or_default();
        Self {
            uri: format!("pubky:{author_id}/pubky.app/posts/{id}"),
            content: node.get("content").unwrap_or_default(),
            id,
            indexed_at: node.get("indexed_at").unwrap_or_default(),
            author: String::from(author_id),
        }
    }

    pub async fn set_index(
        &self,
        author_id: &str,
        post_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let key = format!("{}:{}", author_id, post_id);
        index::set(&Self::prefix(), &key, self, None, None).await
    }

    pub async fn get_from_index(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<Self>, Box<dyn std::error::Error + Send + Sync>> {
        let key = format!("{}:{}", author_id, post_id);
        index::get(&Self::prefix(), &key, None).await
    }

    /// Retrieves the post fields from Neo4j.
    pub async fn get_from_graph(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<PostDetails>, Box<dyn std::error::Error + Send + Sync>> {
        let graph = get_neo4j_graph()?;
        let query = queries::get_post_by_id(author_id, post_id);

        let graph = graph.lock().await;
        let mut result = graph.execute(query).await?;

        match result.next().await? {
            Some(row) => {
                let node: Node = row.get("p").unwrap();
                let post = Self::from_node(&node, author_id).await;
                post.set_index(author_id, post_id).await?;
                Ok(Some(post))
            }
            None => Ok(None),
        }
    }
}
