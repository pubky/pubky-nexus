use crate::db::kv::RedisResult;
use crate::db::{fetch_row_from_graph, queries, RedisOps};
use crate::types::DynError;
use pubky_app_specs::{post_uri_builder, ParsedUri, PubkyAppPost, PubkyAppPostKind, PubkyId};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use utoipa::ToSchema;

mod parsed_uri_option {
    use super::*;

    pub fn serialize<S>(value: &Option<ParsedUri>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value
            .as_ref()
            .and_then(|v| v.try_to_uri_str().ok())
            .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<ParsedUri>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt: Option<String> = Option::deserialize(deserializer)?;
        opt.map(|s| ParsedUri::try_from(s.as_str()).map_err(serde::de::Error::custom))
            .transpose()
    }
}

#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
pub struct PostRelationships {
    /// If set, URI of the post this is a reply to
    #[schema(value_type = Option<String>)]
    #[serde(with = "parsed_uri_option")]
    pub replied: Option<ParsedUri>,

    /// If set, URI of the post this post is reposting
    #[schema(value_type = Option<String>)]
    #[serde(with = "parsed_uri_option")]
    pub reposted: Option<ParsedUri>,

    /// List of user IDs mentioned in this post
    pub mentioned: Vec<PubkyId>,
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
    ) -> RedisResult<Option<PostRelationships>> {
        Self::try_from_index_json(&[author_id, post_id], None).await
    }

    /// Retrieves the counts from Neo4j.
    pub async fn get_from_graph(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<PostRelationships>, DynError> {
        let query = queries::get::post_relationships(author_id, post_id);
        let maybe_row = fetch_row_from_graph(query).await?;

        let Some(row) = maybe_row else {
            return Ok(None);
        };

        let replied_post_id: Option<String> = row.get("replied_post_id").unwrap_or(None);
        let replied_author_id: Option<String> = row.get("replied_author_id").unwrap_or(None);
        let reposted_post_id: Option<String> = row.get("reposted_post_id").unwrap_or(None);
        let reposted_author_id: Option<String> = row.get("reposted_author_id").unwrap_or(None);
        let mentioned: Vec<PubkyId> = row.get("mentioned_user_ids").unwrap_or(Vec::new());

        let replied = replied_author_id
            .zip(replied_post_id)
            .map(|(author_id, post_id)| post_uri_builder(author_id, post_id))
            .and_then(|uri| ParsedUri::try_from(uri).ok());
        let reposted = reposted_author_id
            .zip(reposted_post_id)
            .map(|(author_id, post_id)| post_uri_builder(author_id, post_id))
            .and_then(|uri| ParsedUri::try_from(uri).ok());

        Ok(Some(Self {
            replied,
            reposted,
            mentioned,
        }))
    }

    /// Constructs a `Self` instance by extracting relationships from a `PubkyAppPost` object
    pub fn from_homeserver(post: &PubkyAppPost) -> Self {
        let mut relationship = Self::default();

        if let Some(parent_uri) = &post.parent {
            relationship.replied = ParsedUri::try_from(parent_uri.as_str()).ok()
        }

        if let Some(embed) = &post.embed {
            if let PubkyAppPostKind::Short = embed.kind {
                relationship.reposted = ParsedUri::try_from(embed.uri.as_str()).ok()
            }
        }
        relationship
    }

    pub async fn put_to_index(&self, author_id: &str, post_id: &str) -> RedisResult<()> {
        self.put_index_json(&[author_id, post_id], None, None).await
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
