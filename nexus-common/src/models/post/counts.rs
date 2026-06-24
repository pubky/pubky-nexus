use crate::db::kv::RedisResult;
use crate::db::{fetch_row_from_graph, queries, GraphResult, RedisOps};
use crate::models::error::ModelResult;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::PostStream;

/// Backstop TTL for the read-through cache, in case an invalidation is ever missed.
const POST_COUNTS_TTL_SECS: i64 = 600;

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
    pub async fn get_by_id(author_id: &str, post_id: &str) -> ModelResult<Option<PostCounts>> {
        match Self::get_from_index(author_id, post_id).await? {
            Some(counts) => Ok(Some(counts)),
            None => {
                let graph_response = Self::get_from_graph(author_id, post_id).await?;
                if let Some((post_counts, _is_reply)) = graph_response {
                    // Cache miss: populate from the graph via cache_json (JSON only).
                    post_counts.cache_json(author_id, post_id).await?;
                    return Ok(Some(post_counts));
                }
                Ok(None)
            }
        }
    }

    pub async fn get_from_index(author_id: &str, post_id: &str) -> RedisResult<Option<PostCounts>> {
        Self::try_from_index_json(&[author_id, post_id], None).await
    }

    /// Retrieves the counts from Neo4j.
    pub async fn get_from_graph(
        author_id: &str,
        post_id: &str,
    ) -> GraphResult<Option<(PostCounts, bool)>> {
        let query = queries::get::post_counts(author_id, post_id);
        let maybe_row = fetch_row_from_graph(query).await?;

        if let Some(row) = maybe_row {
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
    ) -> RedisResult<()> {
        self.cache_json(author_id, post_id).await?;

        // Skip the global engagement sorted set for replies. They're tracked
        // via POST_REPLIES sets instead.
        if !is_reply {
            PostStream::add_to_engagement_sorted_set(self, author_id, post_id).await?;
        }
        Ok(())
    }

    /// JSON-only cache write (with TTL). Unlike `put_to_index` it never seeds the
    /// engagement sorted set, so it is safe to call on every cache-miss read.
    pub async fn cache_json(&self, author_id: &str, post_id: &str) -> RedisResult<()> {
        self.put_index_json(&[author_id, post_id], None, Some(POST_COUNTS_TTL_SECS))
            .await
    }

    /// Invalidates the cached counts JSON so the next read recomputes from the
    /// graph (read-your-writes). Does NOT touch the engagement sorted set.
    pub async fn invalidate(index_key: &[&str]) -> RedisResult<()> {
        Self::remove_from_index_multiple_json(&[index_key]).await
    }

    pub async fn reindex(author_id: &str, post_id: &str) -> ModelResult<()> {
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
    ) -> RedisResult<()> {
        // Delete user_details on Redis
        Self::remove_from_index_multiple_json(&[&[author_id, post_id]]).await?;
        // Delete the posts that does not have any relationship as might be replies and reposts. Just root posts
        if remove_from_feeds {
            PostStream::delete_from_engagement_sorted_set(author_id, post_id).await?;
        }
        Ok(())
    }
}
