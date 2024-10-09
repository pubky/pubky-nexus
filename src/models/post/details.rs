use super::PostStream;
use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::db::graph::exec::exec_single_row;
use crate::models::pubky_app::{PostKind, PubkyAppPost};
use crate::models::user::PubkyId;
use crate::{queries, RedisOps};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents post data with content, bio, image, links, and status.
#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
// NOTE: Might not be necessary the default values for serde because before PUT a PostDetails node
// we do sanity check
pub struct PostDetails {
    pub content: String,
    pub id: String,
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
        match Self::get_from_index(author_id, post_id).await? {
            Some(details) => Ok(Some(details)),
            None => {
                let graph_response = Self::get_from_graph(author_id, post_id).await?;
                if let Some(post_details) = graph_response {
                    post_details.put_to_index(author_id).await?;
                    return Ok(Some(post_details));
                }
                Ok(None)
            }
        }
    }

    pub async fn get_from_index(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<PostDetails>, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(post_details) = Self::try_from_index_json(&[author_id, post_id]).await? {
            return Ok(Some(post_details));
        }
        Ok(None)
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
            Some(row) => match row.get("details") {
                Ok(post) => Ok(Some(post)),
                Err(_e) => Ok(None),
            },
            None => Ok(None),
        }
    }

    pub async fn put_to_index(
        &self,
        author_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.put_index_json(&[author_id, &self.id]).await?;
        PostStream::add_to_timeline_sorted_set(self).await?;
        PostStream::add_to_per_user_sorted_set(self).await?;
        Ok(())
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

    pub async fn reindex(
        author_id: &str,
        post_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match Self::get_from_graph(author_id, post_id).await? {
            Some(details) => details.put_to_index(author_id).await?,
            None => log::error!(
                "{}:{} Could not found post counts in the graph",
                author_id,
                post_id
            ),
        }
        Ok(())
    }

    // Save new graph node
    pub async fn put_to_graph(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Save new graph node;
        exec_single_row(queries::put::create_post(self)?).await
    }
}
