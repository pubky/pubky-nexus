//! A marker for a post whose hard delete is in flight.
//!
//! The watcher clears a deleted post from Redis before it deletes the graph
//! node, so a failed delete can be retried. A read that misses Redis in between
//! would find the node still in the graph and cache it again, leaving a post
//! that no longer exists in the index for good. Read-repair writes therefore
//! go through a guard: each one checks this marker in the same atomic step as
//! its write and skips it while the marker exists.
//!
//! Every read-repair write lands either before the marker is set, and the
//! cleanup that follows removes it, or after, and the guard refuses it.
//!
//! - [`PostDeleting::mark_in_flight`] at the start of every delete attempt,
//!   with a TTL that outlasts the longest retry backoff, so the graph node a
//!   failed attempt leaves behind cannot be cached again between retries.
//! - [`PostDeleting::mark_deleted`] once the graph node is gone, followed by a
//!   second cleanup: the marker drops to a short TTL that still refuses a
//!   reader that read the graph just before the delete, and the cleanup
//!   removes anything cached if the marker lapsed anyway.
//!
//! Known gap: trust-filter reconcile writes posts it read from the graph into
//! the shared sets without this guard.

use crate::config::watcher::DEFAULT_MAX_TRANSIENT_BACKOFF_SECS;
use crate::db::get_redis_conn;
use crate::db::kv::RedisResult;
use deadpool_redis::redis;

const POST_DELETING_PREFIX: &str = "Post:Deleting";

/// The marker's TTL while the graph node may still exist: the longest default
/// gap between two retries of a failed delete. An event that is never retried
/// again leaves read-repair off for this post until it expires.
pub const POST_DELETING_IN_FLIGHT_TTL_SECS: u64 = DEFAULT_MAX_TRANSIENT_BACKOFF_SECS;

/// The marker's TTL once the graph node is gone. It must outlast the gap
/// between a reader's graph read and its write.
pub const POST_DELETING_TTL_SECS: u64 = 60;

pub struct PostDeleting;

impl PostDeleting {
    /// The marker key for the post `author_id:post_id`.
    pub fn key(author_id: &str, post_id: &str) -> String {
        format!("{POST_DELETING_PREFIX}:{author_id}:{post_id}")
    }

    /// Sets (or re-arms) the marker for a delete attempt. Call it before the
    /// first Redis cleanup of the post.
    pub async fn mark_in_flight(author_id: &str, post_id: &str) -> RedisResult<()> {
        Self::set(author_id, post_id, POST_DELETING_IN_FLIGHT_TTL_SECS).await
    }

    /// Shortens the marker once the graph node is gone.
    pub async fn mark_deleted(author_id: &str, post_id: &str) -> RedisResult<()> {
        Self::set(author_id, post_id, POST_DELETING_TTL_SECS).await
    }

    async fn set(author_id: &str, post_id: &str, ttl_secs: u64) -> RedisResult<()> {
        let mut conn = get_redis_conn().await?;
        let _: () = redis::cmd("SET")
            .arg(Self::key(author_id, post_id))
            .arg(1)
            .arg("EX")
            .arg(ttl_secs)
            .query_async(&mut conn)
            .await?;
        Ok(())
    }
}
