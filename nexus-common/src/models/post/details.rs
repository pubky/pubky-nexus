use super::{PostRelationships, PostStream};
use crate::db::{exec_single_row, execute_graph_operation, OperationOutcome};
use crate::db::{get_neo4j_graph, queries, RedisOps};
use crate::types::DynError;
use chrono::Utc;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostKind, PubkyId};
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
    pub kind: PubkyAppPostKind,
    pub uri: String,
    pub attachments: Option<Vec<String>>,
}

impl RedisOps for PostDetails {}

impl PostDetails {
    /// Retrieves post details by author ID and post ID, first trying to get from Redis, then from Neo4j if not found.
    pub async fn get_by_id(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<PostDetails>, DynError> {
        match Self::get_from_index(author_id, post_id).await? {
            Some(details) => Ok(Some(details)),
            None => {
                let graph_response = Self::get_from_graph(author_id, post_id).await?;
                if let Some((post_details, reply)) = graph_response {
                    post_details.put_to_index(author_id, reply, false).await?;
                    return Ok(Some(post_details));
                }
                Ok(None)
            }
        }
    }

    pub async fn get_from_index(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<PostDetails>, DynError> {
        if let Some(mut post_details) =
            Self::try_from_index_json(&[author_id, post_id], None).await?
        {
            // Normalize attachments: treat `None` the same as an empty Vec
            // TODO: Since an empty Vec already conveys “no attachments,” we don’t actually need an Option.
            // Move this normalization into post creation, `PostDetails::from_homeserver()`
            // so that model property is always created with `attachments = Vec::new()`
            if post_details.attachments.is_none() {
                post_details.attachments = Some(Vec::new());
            }
            return Ok(Some(post_details));
        }
        Ok(None)
    }

    /// Retrieves the post fields from Neo4j.
    pub async fn get_from_graph(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<(PostDetails, Option<(String, String)>)>, DynError> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query = queries::get::get_post_by_id(author_id, post_id);

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        match result.next().await? {
            Some(row) => {
                let post: PostDetails = row.get("details")?;
                let reply_value: Vec<(String, String)> = row.get("reply").unwrap_or(Vec::new());
                let reply_key = match reply_value.is_empty() {
                    true => None,
                    false => Some(reply_value[0].clone()),
                };
                Ok(Some((post, reply_key)))
            }
            None => Ok(None),
        }
    }

    pub async fn put_to_index(
        &self,
        author_id: &str,
        parent_key_wrapper: Option<(String, String)>,
        is_edit: bool,
    ) -> Result<(), DynError> {
        self.put_index_json(&[author_id, &self.id], None, None)
            .await?;
        // When we delete a post that has ancestor, ignore other index updates
        if is_edit {
            return Ok(());
        }
        // The replies are not indexed in the global feeds so we will ignore that indexing
        match parent_key_wrapper {
            None => {
                PostStream::add_to_timeline_sorted_set(self).await?;
                PostStream::add_to_per_user_sorted_set(self).await?;
            }
            Some((parent_author_id, parent_post_id)) => {
                PostStream::add_to_post_reply_sorted_set(
                    &[&parent_author_id, &parent_post_id],
                    author_id,
                    &self.id,
                    self.indexed_at,
                )
                .await?;
                PostStream::add_to_replies_per_user_sorted_set(self).await?;
            }
        }
        Ok(())
    }

    pub async fn from_homeserver(
        homeserver_post: PubkyAppPost,
        author_id: &PubkyId,
        post_id: &String,
    ) -> Result<Self, DynError> {
        Ok(PostDetails {
            uri: format!("pubky://{author_id}/pub/pubky.app/posts/{post_id}"),
            content: homeserver_post.content,
            id: post_id.clone(),
            indexed_at: Utc::now().timestamp_millis(),
            author: author_id.to_string(),
            kind: homeserver_post.kind,
            // TODO: eliminate Option for attachments and always use Vec<String>
            // Once PubkyAppPost.attachments is Vec<T>, we can drop this Option entirely
            // Check above TODO in get_from_index()
            attachments: homeserver_post.attachments,
        })
    }

    pub async fn reindex(author_id: &str, post_id: &str) -> Result<(), DynError> {
        match Self::get_from_graph(author_id, post_id).await? {
            Some((details, reply)) => details.put_to_index(author_id, reply, false).await?,
            None => tracing::error!(
                "{}:{} Could not found post counts in the graph",
                author_id,
                post_id
            ),
        }
        Ok(())
    }

    // Save new graph node
    pub async fn put_to_graph(
        &self,
        post_relationships: &PostRelationships,
    ) -> Result<OperationOutcome, DynError> {
        match queries::put::create_post(self, post_relationships) {
            Ok(query) => execute_graph_operation(query).await,
            Err(_) => Err("QUERY: Error while creating the query".into()),
        }
    }

    pub async fn delete(
        author_id: &str,
        post_id: &str,
        parent_post_key_wrapper: Option<[String; 2]>,
    ) -> Result<(), DynError> {
        // Delete user_details on Redis
        Self::remove_from_index_multiple_json(&[&[author_id, post_id]]).await?;
        // Delete post graph node
        exec_single_row(queries::del::delete_post(author_id, post_id)).await?;
        // The replies are not indexed in the global feeds
        match parent_post_key_wrapper {
            None => {
                PostStream::remove_from_timeline_sorted_set(author_id, post_id).await?;
                PostStream::remove_from_per_user_sorted_set(author_id, post_id).await?;
            }
            Some([parent_author_id, parent_post_id]) => {
                PostStream::remove_from_post_reply_sorted_set(
                    &[&parent_author_id, &parent_post_id],
                    author_id,
                    post_id,
                )
                .await?;
                PostStream::remove_from_replies_per_user_sorted_set(author_id, post_id).await?;
            }
        }
        Ok(())
    }
}
