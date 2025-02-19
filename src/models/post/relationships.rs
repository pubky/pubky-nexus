use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::types::DynError;
use crate::{queries, RedisOps};
use pubky_app_specs::{PubkyAppPost, PubkyAppPostKind};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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
    ) -> Result<Option<PostRelationships>, DynError> {
        match Self::get_from_index(author_id, post_id).await? {
            Some(counts) => Ok(Some(counts)),
            None => {
                let graph_response = Self::get_from_graph(author_id, post_id).await?;
                if let Some(post_relationships) = graph_response {
                    post_relationships.put_to_index(author_id, post_id).await?;
                    return Ok(Some(post_relationships));
                }
                Ok(None)
            }
        }
    }

    pub async fn get_from_index(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<PostRelationships>, DynError> {
        if let Some(post_relationships) =
            Self::try_from_index_json(&[author_id, post_id], None).await?
        {
            return Ok(Some(post_relationships));
        }
        Ok(None)
    }

    /// Retrieves the counts from Neo4j.
    pub async fn get_from_graph(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<PostRelationships>, DynError> {
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
            Ok(Some(Self {
                replied,
                reposted,
                mentioned,
            }))
        } else {
            Ok(None)
        }
    }

    /// Constructs a `Self` instance by extracting relationships from a `PubkyAppPost` object
    pub fn from_homeserver(post: &PubkyAppPost) -> Self {
        let mut relationship = Self::default();

        if let Some(parent_uri) = &post.parent {
            relationship.replied = Some(parent_uri.to_string());
        }

        if let Some(embed) = &post.embed {
            if let PubkyAppPostKind::Short = embed.kind {
                relationship.reposted = Some(embed.uri.clone());
            }
        }
        relationship
    }

    pub async fn put_to_index(&self, author_id: &str, post_id: &str) -> Result<(), DynError> {
        self.put_index_json(&[author_id, post_id], None, None)
            .await?;
        Ok(())
    }

    pub async fn delete(author_id: &str, post_id: &str) -> Result<(), DynError> {
        Self::remove_from_index_multiple_json(&[&[author_id, post_id]]).await?;
        Ok(())
    }

    pub async fn reindex(author_id: &str, post_id: &str) -> Result<(), DynError> {
        match Self::get_from_graph(author_id, post_id).await? {
            Some(relationships) => relationships.put_to_index(author_id, post_id).await?,
            None => tracing::error!(
                "{}:{} Could not found post relationships in the graph",
                author_id,
                post_id
            ),
        }
        Ok(())
    }
}
