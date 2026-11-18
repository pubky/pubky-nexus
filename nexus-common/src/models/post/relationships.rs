use crate::db::kv::RedisResult;
use crate::db::{fetch_row_from_graph, queries, GraphResult, RedisOps};
use crate::models::error::ModelResult;
use crate::models::ResourceKey;
use pubky_social_specs::PubkyId;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
pub struct PostRelationships {
    /// If set, URI of what this post replies to: a post, or an external resource (another
    /// scheme's URL or another app's public object).
    pub replied: Option<String>,

    /// Whether `replied` names an external resource rather than a post, as the graph read it:
    /// the post's `REPLIED` edge ends on a Resource node. `/v0` names posts only and reads
    /// `null` for any other reply. A cached value written before the flag existed reads
    /// `false`, which is right for it: `replied` then only ever named a post.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub replied_external: bool,

    /// If set, URI of the post this post is reposting
    pub reposted: Option<String>,

    /// External URI embedded by this post. Native post embeds use `reposted`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedded: Option<String>,

    /// List of user IDs mentioned in this post
    pub mentioned: Vec<PubkyId>,
}

impl RedisOps for PostRelationships {}

impl PostRelationships {
    /// Retrieves post relationships from Redis, or from the graph on a miss.
    /// A miss is not written back, as in [`super::PostDetails::get_by_id`].
    pub async fn get_by_id(
        author_id: &str,
        post_id: &str,
    ) -> ModelResult<Option<PostRelationships>> {
        match Self::get_from_index(author_id, post_id).await? {
            Some(relationships) => Ok(Some(relationships)),
            None => Ok(Self::get_from_graph(author_id, post_id).await?),
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

        let replied = row.get::<Option<String>>("replied_uri").unwrap_or(None);
        let replied_external = row.get::<bool>("replied_external").unwrap_or(false);
        let reposted = row.get::<Option<String>>("reposted_uri").unwrap_or(None);
        let mentioned: Vec<PubkyId> = row.get("mentioned_user_ids").unwrap_or(Vec::new());

        Ok(Some(Self {
            replied,
            replied_external,
            reposted,
            embedded: row.get("embedded_uri").unwrap_or(None),
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

/// A post this post replies to or reposts: the address stored on that post and its key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostParent {
    pub uri: String,
    pub key: ResourceKey,
}

/// The posts a post replies to and reposts, read from its `REPLIED` and `REPOSTED` edges.
///
/// The graph holds the parents' keys whatever epoch their addresses are written in, so delete,
/// tombstone and recovery read them here instead of parsing a stored address. A post holds at
/// most one of each, set when it is created: the watcher rejects an edit that changes either.
/// A reply to an external target (another scheme's URL, another app's object) has no post
/// parent but a Resource reply edge, fixed the same way once the post's references were first
/// reconciled.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PostParents {
    pub replied: Option<PostParent>,
    pub reposted: Option<PostParent>,
    /// The id of the Resource the post replies to: the graph's Resource reply edge, or for an
    /// incoming post the normalized id of the external target its parent names.
    pub replied_resource: Option<String>,
    /// Whether the post's external references were ever reconciled
    /// (`references::stage` writes its `resource_ref_revision` with its edges). A post indexed
    /// before posts referenced Resources was not: it has no Resource reply edge yet, and its
    /// next PUT writes the one it names.
    pub references_reconciled: bool,
}

impl PostParents {
    /// `None` when the graph has no such post.
    pub async fn get_from_graph(author_id: &str, post_id: &str) -> GraphResult<Option<Self>> {
        let query = queries::get::post_parents(author_id, post_id);
        let Some(row) = fetch_row_from_graph(query).await? else {
            return Ok(None);
        };

        let parent = |prefix: &str| -> GraphResult<Option<PostParent>> {
            let uri = row.get::<Option<String>>(&format!("{prefix}_uri"))?;
            let owner_id = row.get::<Option<PubkyId>>(&format!("{prefix}_author_id"))?;
            let resource_id = row.get::<Option<String>>(&format!("{prefix}_post_id"))?;
            Ok(match (uri, owner_id, resource_id) {
                (Some(uri), Some(owner_id), Some(resource_id)) => Some(PostParent {
                    uri,
                    key: ResourceKey {
                        owner_id,
                        resource_id,
                    },
                }),
                _ => None,
            })
        };

        Ok(Some(Self {
            replied: parent("replied")?,
            reposted: parent("reposted")?,
            replied_resource: row.get("replied_resource_id")?,
            references_reconciled: row.get("references_reconciled")?,
        }))
    }
}
