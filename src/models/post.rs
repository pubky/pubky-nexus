use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::{index, prefix, queries};
use chrono::Utc;
use neo4rs::Node;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents post data with content, bio, image, links, and status.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct Post {
    content: String,
    id: String, // TODO: create Crockfordbase32 Struct and validator
    timestamp: i64,
    author: String, // TODO: PubkyKey struct with validator
    uri: String,
}

impl Default for Post {
    fn default() -> Self {
        Self::new()
    }
}

impl Post {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            id: String::new(),
            timestamp: Utc::now().timestamp(),
            author: String::new(),
            uri: String::new(),
        }
    }

    /// Retrieves post details by author ID and post ID, first trying to get from Redis, then from Neo4j if not found.
    pub async fn get_by_id(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<Post>, Box<dyn std::error::Error + Send + Sync>> {
        match Post::get_from_index(author_id, post_id).await? {
            Some(details) => Ok(Some(details)),
            None => Post::get_from_graph(author_id, post_id).await,
        }
    }

    fn from_node(node: &Node, author_id: &str) -> Self {
        let id = node.get("id").unwrap_or_default();
        Self {
            uri: format!("pubky:{author_id}/pubky.app/posts/{id}"),
            content: node.get("content").unwrap_or_default(),
            id,
            timestamp: node.get("timestamp").unwrap_or_default(),
            author: String::from(author_id),
        }
    }

    pub async fn set_index(
        &self,
        author_id: &str,
        post_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let key = format!("{}:{}", author_id, post_id);
        index::set(prefix::POST, &key, self, None, None).await
    }

    pub async fn get_from_index(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<Self>, Box<dyn std::error::Error + Send + Sync>> {
        let key = format!("{}:{}", author_id, post_id);
        index::get(prefix::POST, &key, None).await
    }

    /// Retrieves the post fields from Neo4j.
    pub async fn get_from_graph(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<Post>, Box<dyn std::error::Error + Send + Sync>> {
        let graph = get_neo4j_graph()?;
        let query = queries::get_post_by_id(author_id, post_id);

        let graph = graph.lock().await;
        let mut result = graph.execute(query).await?;

        if let Some(row) = result.next().await? {
            let node: Node = row.get("p").unwrap();
            let post = Post::from_node(&node, author_id);
            post.set_index(author_id, post_id).await?;
            Ok(Some(post))
        } else {
            Ok(None)
        }
    }
}
