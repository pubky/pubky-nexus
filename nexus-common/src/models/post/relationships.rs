use crate::db::kv::RedisResult;
use crate::db::{fetch_row_from_graph, queries, GraphResult, RedisOps};
use crate::models::error::ModelResult;
use pubky_social_specs::legacy_v0::ParsedUri;
use pubky_social_specs::PubkyId;
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

// TODO(specs-migration): `ParsedUri` only parses and renders `pubky.app`, so a parent stored
// under `social/v1` is dropped when read from the graph. The specs migration needs an internal
// function that reads the owner and post id from an address in either epoch.
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
    ) -> ModelResult<Option<PostRelationships>> {
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
    ) -> GraphResult<Option<PostRelationships>> {
        let query = queries::get::post_relationships(author_id, post_id);
        let maybe_row = fetch_row_from_graph(query).await?;

        let Some(row) = maybe_row else {
            return Ok(None);
        };

        let parent = |column: &str| {
            row.get::<Option<String>>(column)
                .unwrap_or(None)
                .and_then(|uri| ParsedUri::try_from(uri).ok())
        };
        let replied = parent("replied_uri");
        let reposted = parent("reposted_uri");
        let mentioned: Vec<PubkyId> = row.get("mentioned_user_ids").unwrap_or(Vec::new());

        Ok(Some(Self {
            replied,
            reposted,
            mentioned,
        }))
    }

    pub async fn put_to_index(&self, author_id: &str, post_id: &str) -> RedisResult<()> {
        self.put_index_json(&[author_id, post_id], None, None).await
    }

    pub async fn delete(author_id: &str, post_id: &str) -> RedisResult<()> {
        Self::remove_from_index_multiple_json(&[&[author_id, post_id]]).await
    }

    pub async fn reindex(author_id: &str, post_id: &str) -> ModelResult<()> {
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
