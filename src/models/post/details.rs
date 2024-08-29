use super::PostStream;
use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::db::graph::exec::exec_single_row;
use crate::models::pubky_app::{PostKind, PubkyAppPost};
use crate::models::user::PubkyId;
use crate::{queries, RedisOps};
use chrono::Utc;
use neo4rs::Node;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents post data with content, bio, image, links, and status.
#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
pub struct PostDetails {
    pub content: String,
    pub id: String, // TODO: create Crockfordbase32 validator
    pub indexed_at: i64,
    pub author: String,
    pub kind: PostKind,
    pub uri: String,
}

impl RedisOps for PostDetails {}

impl PostDetails {
    /// Retrieves post details by author ID and post ID, first trying to get from Redis, then from Neo4j if not found.
    pub async fn get_by_id(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<PostDetails>, Box<dyn std::error::Error + Send + Sync>> {
        match Self::try_from_index_json(&[author_id, post_id]).await? {
            Some(details) => Ok(Some(details)),
            None => Self::get_from_graph(author_id, post_id).await,
        }
    }

    async fn from_node(node: &Node, author_id: &str) -> Self {
        let id = node.get("id").unwrap_or_default();
        Self {
            uri: format!("pubky://{author_id}/pub/pubky.app/posts/{id}"),
            content: node.get("content").unwrap_or_default(),
            id,
            indexed_at: node.get("indexed_at").unwrap_or_default(),
            author: String::from(author_id),
            kind: node.get("kind").unwrap_or_default(),
        }
    }

    pub async fn from_homeserver(
        homeserver_post: PubkyAppPost,
        author_id: &PubkyId,
        post_id: &String,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(PostDetails {
            uri: format!("pubky://{author_id}/pub/pubky.app/posts/{post_id}"),
            content: homeserver_post.content,
            id: post_id.clone(),
            indexed_at: Utc::now().timestamp_millis(),
            author: author_id.0.clone(),
            kind: homeserver_post.kind,
        })
    }

    // Save new graph node
    pub async fn save(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Save new graph node;
        exec_single_row(queries::write::create_post(self)?).await
    }

    /// Retrieves the post fields from Neo4j.
    pub async fn get_from_graph(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<PostDetails>, Box<dyn std::error::Error + Send + Sync>> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query = queries::read::get_post_by_id(author_id, post_id);

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        match result.next().await? {
            Some(row) => {
                let node: Node = row.get("p")?;
                let post = Self::from_node(&node, author_id).await;
                post.put_index_json(&[author_id, post_id]).await?;
                PostStream::add_to_timeline_sorted_set(&post).await?;
                PostStream::add_to_per_user_sorted_set(&post).await?;
                Ok(Some(post))
            }
            None => Ok(None),
        }
    }
}
