use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::events::uri::ParsedUri;
use crate::models::user::PubkyId;
use crate::{queries, RedisOps};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::PostStream;

#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
pub struct PostRelationships {
    // URI of the replied post
    pub replied: Option<String>,
    // URI of the reposted post
    pub reposted: Option<String>,
    // List of user IDs
    pub mentioned: Vec<String>,
}

impl RedisOps for PostRelationships {}

impl PostRelationships {
    /// Retrieves post relationships by user ID, first trying to get from Redis, then from Neo4j if not found.
    pub async fn get_by_id(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<PostRelationships>, Box<dyn std::error::Error + Send + Sync>> {
        match Self::get_from_index(author_id, post_id).await? {
            Some(counts) => Ok(Some(counts)),
            None => {
                let graph_response = Self::get_from_graph(author_id, post_id).await?;
                if let Some((post_relationships, indexed_at)) = graph_response {
                    post_relationships.put_to_index(author_id, post_id, indexed_at).await?;
                    return Ok(Some(post_relationships));
                }
                Ok(None)
            }
        }
    }

    pub async fn get_from_index(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<PostRelationships>, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(post_relationships) = Self::try_from_index_json(&[author_id, post_id]).await? {
            return Ok(Some(post_relationships));
        }
        Ok(None)
    }

    /// Retrieves the counts from Neo4j.
    pub async fn get_from_graph(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<(PostRelationships, i64)>, Box<dyn std::error::Error + Send + Sync>> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query = queries::get::post_relationships(author_id, post_id);

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        if let Some(row) = result.next().await? {
            let replied_post_id: Option<String> = row.get("replied_post_id").unwrap_or(None);
            let replied_author_id: Option<String> = row.get("replied_author_id").unwrap_or(None);
            let reposted_post_id: Option<String> = row.get("reposted_post_id").unwrap_or(None);
            let reposted_author_id: Option<String> = row.get("reposted_author_id").unwrap_or(None);
            let mentioned: Vec<String> = row.get("mentioned_user_ids").unwrap_or(Vec::new());
            let indexed_at: i64 = row.get("indexed_at").unwrap_or(Utc::now().timestamp());

            let replied = match (replied_author_id, replied_post_id) {
                (Some(author_id), Some(post_id)) => {
                    Some(format!("pubky://{author_id}/pub/pubky.app/posts/{post_id}"))
                }
                _ => None,
            };
            let reposted = match (reposted_author_id, reposted_post_id) {
                (Some(author_id), Some(post_id)) => {
                    Some(format!("pubky://{author_id}/pub/pubky.app/posts/{post_id}"))
                }
                _ => None,
            };
            Ok(Some(
                (Self {
                    replied,
                    reposted,
                    mentioned,
                }, 
                indexed_at)
            ))
        } else {
            Ok(None)
        }
    }

    pub async fn put_to_index(
        &self,
        author_id: &str,
        post_id: &str,
        indexed_at: i64
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.put_index_json(&[author_id, post_id]).await?;
        if let Some((parent_author_id, parent_post_id)) = self.is_reply() {
            PostStream::add_to_post_reply_sorted_set(
                &parent_author_id,
                &parent_post_id,
                author_id,
                post_id,
                indexed_at
            ).await?;
        }
        Ok(())
    }

    // Check if that post has a parent, like that might be a reply
    fn is_reply(&self) -> Option<(PubkyId, String)> {
        if let Some(replied_uri) = self.replied.as_ref() {
                let parsed_uri = ParsedUri::try_from(replied_uri.as_str()).unwrap();
                let parsed_uri_author_id = parsed_uri.user_id;
                let parsed_uri_post_id = parsed_uri.post_id.unwrap();
                return Some((parsed_uri_author_id, parsed_uri_post_id));
        }
        None
    }

    pub async fn reindex(
        author_id: &str,
        post_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match Self::get_from_graph(author_id, post_id).await? {
            Some((relationships, indexed_at)) => relationships.put_to_index(author_id, post_id, indexed_at).await?,
            None => log::error!(
                "{}:{} Could not found post relationships in the graph",
                author_id,
                post_id
            ),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{setup, Config};

    use super::*;

    const AUTHOR_ID: &str = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";
    const POST_ID: &str = "2ZCWWEQ4TB600";

    const USER_ID: &str = "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao";
    const REPLY_ID: &str = "2ZCWXSXM1FHG0";

    #[tokio::test]
    async fn test_reply_get_by_id_fn() {

        // Open connections against ddbb
        let config = Config::from_env();
        setup(&config).await;

        let parent_post_res = PostRelationships::get_by_id(AUTHOR_ID, POST_ID).await.unwrap();
        assert!(parent_post_res.is_some(), "Post has to exist");

        let parent_post = parent_post_res.unwrap();
        assert!(parent_post.replied.is_none());
        assert!(parent_post.reposted.is_none());

        let reply_post_res = PostRelationships::get_by_id(USER_ID, REPLY_ID).await.unwrap();
        assert!(reply_post_res.is_some(), "The post has to be a reply");

        let reply_post = reply_post_res.unwrap();

        if let Some((author_id, post_id)) = reply_post.is_reply() {
            assert_eq!(author_id.as_str(), AUTHOR_ID);
            assert_eq!(post_id.as_str(), POST_ID);
        } else {
            assert!(false)
        }
    }
}
