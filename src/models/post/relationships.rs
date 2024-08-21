use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::{queries, RedisOps};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema, Default)]
pub struct PostRelationships {
    // URI of the replied post
    replied: Option<String>,
    // URI of the reposted post
    reposted: Option<String>,
    // List of user IDs
    mentioned: Option<Vec<String>>,
}

impl RedisOps for PostRelationships {}

impl PostRelationships {
    /// Retrieves post relationships by user ID, first trying to get from Redis, then from Neo4j if not found.
    pub async fn get_by_id(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<PostRelationships>, Box<dyn std::error::Error + Send + Sync>> {
        match Self::try_from_index_json(&[author_id, post_id]).await? {
            Some(counts) => Ok(Some(counts)),
            None => Self::get_from_graph(author_id, post_id).await,
        }
    }

    /// Retrieves the counts from Neo4j.
    pub async fn get_from_graph(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<PostRelationships>, Box<dyn std::error::Error + Send + Sync>> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query = queries::post_relationships(author_id, post_id);

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        if let Some(row) = result.next().await? {
            let replied_post_id: Option<String> = row.get("replied_post_id")?;
            let replied_author_id: Option<String> = row.get("replied_author_id")?;
            let reposted_post_id: Option<String> = row.get("reposted_post_id")?;
            let reposted_author_id: Option<String> = row.get("reposted_author_id")?;
            let mentioned: Option<Vec<String>> = row.get("mentioned_user_ids")?;

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
            let relationships = Self {
                replied,
                reposted,
                mentioned,
            };
            relationships.put_index_json(&[author_id, post_id]).await?;
            Ok(Some(relationships))
        } else {
            Ok(None)
        }
    }
}
