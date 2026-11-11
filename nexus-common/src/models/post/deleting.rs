//! A marker for a post whose hard delete is in flight.
//!
//! `sync_del` clears a post from Redis before it deletes the graph node, so a
//! failed delete can be retried. A read that misses Redis in between would
//! find the node and cache it again, for good. Read-repair writes therefore
//! check this marker in the same atomic step as the write and skip while it
//! exists: a write lands either before the marker, and the cleanup after it
//! removes it, or after, and is refused.
//!
//! The marker is set with a TTL past the longest retry backoff at the start of
//! every delete attempt, then shortened once the graph node is gone. It still
//! refuses a reader that read the graph just before the delete; the cleanup
//! that follows removes anything cached if it lapsed anyway.
//!
//! Known gap: trust-filter reconcile writes posts it read from the graph into
//! the shared sets without this guard.

use crate::config::watcher::DEFAULT_MAX_TRANSIENT_BACKOFF_SECS;
use crate::db::get_redis_conn;
use crate::db::kv::RedisResult;
use deadpool_redis::redis::AsyncCommands;

/// The marker's TTL while the graph node may still exist: the longest default
/// gap between two retries of a failed delete.
const POST_DELETING_IN_FLIGHT_TTL_SECS: u64 = DEFAULT_MAX_TRANSIENT_BACKOFF_SECS;

/// The marker's TTL once the graph node is gone. It must outlast the gap
/// between a reader's graph read and its write.
pub const POST_DELETING_TTL_SECS: u64 = 60;

pub struct PostDeleting;

impl PostDeleting {
    pub fn key(author_id: &str, post_id: &str) -> String {
        format!("Post:Deleting:{author_id}:{post_id}")
    }

    /// Sets the marker for a delete attempt, before its first Redis cleanup.
    pub async fn mark_in_flight(author_id: &str, post_id: &str) -> RedisResult<()> {
        Self::set(author_id, post_id, POST_DELETING_IN_FLIGHT_TTL_SECS).await
    }

    /// Shortens the marker once the graph node is gone.
    pub async fn mark_deleted(author_id: &str, post_id: &str) -> RedisResult<()> {
        Self::set(author_id, post_id, POST_DELETING_TTL_SECS).await
    }

    async fn set(author_id: &str, post_id: &str, ttl_secs: u64) -> RedisResult<()> {
        let mut conn = get_redis_conn().await?;
        let _: () = conn
            .set_ex(Self::key(author_id, post_id), 1, ttl_secs)
            .await?;
        Ok(())
    }
}
