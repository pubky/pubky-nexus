//! A short-lived marker for a post whose hard delete is in flight.
//!
//! The watcher clears a deleted post from Redis before it deletes the graph
//! node, so a failed delete can be retried. A read that misses Redis in between
//! would find the node still in the graph and cache it again, leaving a post
//! that no longer exists in the index for good. Read-repair writes therefore
//! go through a guard: each one checks this marker in the same atomic step as
//! its write and skips it while the marker exists.
//!
//! Every read-repair write lands either before the marker is set, and the
//! cleanup that follows removes it, or after, and the guard refuses it. The
//! marker expires on its own instead of being cleared, so a reader that read
//! the graph just before the node was deleted is still refused afterwards.

use crate::db::get_redis_conn;
use crate::db::kv::RedisResult;
use deadpool_redis::redis;

const POST_DELETING_PREFIX: &str = "Post:Deleting";

/// How long read-repair stays off for a deleted post. It must outlast the
/// delete itself plus the gap between a reader's graph read and its write.
pub const POST_DELETING_TTL_SECS: u64 = 60;

pub struct PostDeleting;

impl PostDeleting {
    /// The marker key for the post `author_id:post_id`.
    pub fn key(author_id: &str, post_id: &str) -> String {
        format!("{POST_DELETING_PREFIX}:{author_id}:{post_id}")
    }

    /// Sets (or re-arms) the marker. Call it before the first Redis cleanup of
    /// the post.
    pub async fn mark(author_id: &str, post_id: &str) -> RedisResult<()> {
        let mut conn = get_redis_conn().await?;
        let _: () = redis::cmd("SET")
            .arg(Self::key(author_id, post_id))
            .arg(1)
            .arg("EX")
            .arg(POST_DELETING_TTL_SECS)
            .query_async(&mut conn)
            .await?;
        Ok(())
    }
}
