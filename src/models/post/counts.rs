use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::db::kv::index::json::JsonAction;
use crate::models::tag::post::POST_TAGS_KEY_PARTS;
use crate::types::DynError;
use crate::{queries, RedisOps};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::PostStream;

/// Represents total counts of relationships of a user.
#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
pub struct PostCounts {
    // how many times was pointed the post with a tag
    pub tags: u32,
    // Distinct tags where the post was referenced
    pub unique_tags: u32,
    pub replies: u32,
    pub reposts: u32,
}

impl RedisOps for PostCounts {}

impl PostCounts {
    /// Retrieves counts by user ID, first trying to get from Redis, then from Neo4j if not found.
    pub async fn get_by_id(author_id: &str, post_id: &str) -> Result<Option<PostCounts>, DynError> {
        // TODO: uncomment the get_from_index approach when index counting is stable

        // match Self::get_from_index(author_id, post_id).await? {
        //     Some(counts) => Ok(Some(counts)),
        //     None => {
        //         let graph_response = Self::get_from_graph(author_id, post_id).await?;
        //         if let Some((post_counts, is_reply)) = graph_response {
        //             post_counts
        //                 .put_to_index(author_id, post_id, !is_reply)
        //                 .await?;
        //             return Ok(Some(post_counts));
        //         }
        //         Ok(None)
        //     }
        // }

        if let Some((post_counts, _)) = Self::get_from_graph(author_id, post_id).await? {
            Ok(Some(post_counts))
        } else {
            Ok(None)
        }
    }

    pub async fn get_from_index(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<PostCounts>, DynError> {
        if let Some(post_counts) = Self::try_from_index_json(&[author_id, post_id], None).await? {
            return Ok(Some(post_counts));
        }
        Ok(None)
    }

    /// Retrieves the counts from Neo4j.
    pub async fn get_from_graph(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<(PostCounts, bool)>, DynError> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query = queries::get::post_counts(author_id, post_id);

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        if let Some(row) = result.next().await? {
            let post_exists: bool = row.get("exists").unwrap_or(false);
            if post_exists {
                let counts: PostCounts = row.get("counts")?;
                let is_reply: bool = row.get("is_reply").unwrap_or(false);

                return Ok(Some((counts, is_reply)));
            }
        }
        Ok(None)
    }

    pub async fn put_to_index(
        &self,
        author_id: &str,
        post_id: &str,
        is_reply: bool,
    ) -> Result<(), DynError> {
        self.put_index_json(&[author_id, post_id], None, None)
            .await?;

        // avoid indexing replies into global feeds
        if !is_reply {
            PostStream::add_to_engagement_sorted_set(self, author_id, post_id).await?;
        }
        Ok(())
    }

    /// Updates a specified JSON field in the index
    ///
    /// # Arguments
    ///
    /// * `index_key` - A slice of string references representing the index key parts.
    /// * `field` - The name of the JSON field to be updated.
    /// * `action` - The action to perform on the JSON field (increment or decrement).
    /// * `tag_label` - An optional tag label used to check membership in a sorted set. Important field to update the unique_tags field
    pub async fn update_index_field(
        index_key: &[&str],
        field: &str,
        action: JsonAction,
        tag_label: Option<&str>,
    ) -> Result<(), DynError> {
        // This condition applies only when updating `unique_tags`
        if let Some(label) = tag_label {
            let index_parts = [&POST_TAGS_KEY_PARTS[..], index_key].concat();
            let score = Self::check_sorted_set_member(None, &index_parts, &[label]).await?;
            match (score, &action) {
                // to decrement `unique_tags`, the tag value must be less than or equal to 1
                (Some(tag_value), JsonAction::Decrement(_)) if tag_value < 1 => (),
                // to increment `unique_tags`, the tag must not exist in the sorted set
                (None, JsonAction::Increment(_)) => (),
                // Do not update the index
                _ => return Ok(()),
            }
        }

        Self::modify_json_field(index_key, field, action).await?;
        Ok(())
    }

    pub async fn reindex(author_id: &str, post_id: &str) -> Result<(), DynError> {
        match Self::get_from_graph(author_id, post_id).await? {
            Some((counts, is_reply)) => counts.put_to_index(author_id, post_id, is_reply).await?,
            None => tracing::error!(
                "{}:{} Could not found post counts in the graph",
                author_id,
                post_id
            ),
        }
        Ok(())
    }

    pub async fn delete(
        author_id: &str,
        post_id: &str,
        remove_from_feeds: bool,
    ) -> Result<(), DynError> {
        // Delete user_details on Redis
        Self::remove_from_index_multiple_json(&[&[author_id, post_id]]).await?;
        // Delete the posts that does not have any relationship as might be replies and reposts. Just root posts
        if remove_from_feeds {
            PostStream::delete_from_engagement_sorted_set(author_id, post_id).await?;
        }
        Ok(())
    }
}
