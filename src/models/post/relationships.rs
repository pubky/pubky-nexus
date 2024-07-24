use crate::{db::connectors::neo4j::get_neo4j_graph, index, models::Prefix, queries};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct PostRelationships {
    // URI of the replied post
    replied: Option<String>,
    // URI of the reposted post
    reposted: Option<String>,
    // List of user IDs
    mentioned: Option<Vec<String>>,
}

impl Default for PostRelationships {
    fn default() -> Self {
        Self::new()
    }
}

impl PostRelationships {
    pub fn new() -> Self {
        Self {
            replied: None,
            reposted: None,
            mentioned: None,
        }
    }

    /// Retrieves post relationships by user ID, first trying to get from Redis, then from Neo4j if not found.
    pub async fn get_by_id(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<PostRelationships>, Box<dyn std::error::Error + Send + Sync>> {
        match Self::get_from_index(author_id, post_id).await? {
            Some(counts) => Ok(Some(counts)),
            None => Self::get_from_graph(author_id, post_id).await,
        }
    }

    /// Sets counts in the Redis cache.
    pub async fn set_index(
        &self,
        author_id: &str,
        post_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let key = format!("{}:{}", author_id, post_id);
        index::set(&Self::prefix(), &key, self, None, None).await
    }

    /// Get counts from the Redis cache.
    pub async fn get_from_index(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<Self>, Box<dyn std::error::Error + Send + Sync>> {
        let key = format!("{}:{}", author_id, post_id);
        index::get(&Self::prefix(), &key, None).await
    }

    /// Retrieves the counts from Neo4j.
    pub async fn get_from_graph(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<PostRelationships>, Box<dyn std::error::Error + Send + Sync>> {
        let graph = get_neo4j_graph()?;
        let query = queries::post_relationships(author_id, post_id);

        let graph = graph.lock().await;
        let mut result = graph.execute(query).await?;

        if let Some(row) = result.next().await? {
            let replied_post_id: Option<String> = row.get("replied_post_id")?;
            let replied_author_id: Option<String> = row.get("replied_author_id")?;
            let reposted_post_id: Option<String> = row.get("reposted_post_id")?;
            let reposted_author_id: Option<String> = row.get("reposted_author_id")?;
            let mentioned: Option<Vec<String>> = row.get("mentioned_user_ids")?;

            let replied = match (replied_author_id, replied_post_id) {
                (Some(author_id), Some(post_id)) => {
                    Some(format!("pubky:{author_id}/pubky.app/posts/{post_id}"))
                }
                _ => None,
            };
            let reposted = match (reposted_author_id, reposted_post_id) {
                (Some(author_id), Some(post_id)) => {
                    Some(format!("pubky:{author_id}/pubky.app/posts/{post_id}"))
                }
                _ => None,
            };
            let relationships = Self {
                replied,
                reposted,
                mentioned,
            };
            relationships.set_index(author_id, post_id).await?;
            Ok(Some(relationships))
        } else {
            Ok(None)
        }
    }
}
