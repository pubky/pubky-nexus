//! Posts, tags and files the moderator took down, so they stay down whatever order
//! events arrive in. Profiles aren't remembered: a moderated user may come back by
//! writing a new profile at the same address.
//!
//! The moderator's tag deletes its target when the tag is read. If the target isn't
//! indexed yet (a new Nexus reading everyone's history from the start, in no
//! particular order), there is nothing to delete, and the target would be indexed
//! when its own event is read later. Remembering every taken-down address closes
//! that gap: an event for one is dropped, however late it arrives.
//!
//! The graph (`:Takedown` nodes) is the source of truth; a Redis set mirrors it so the
//! check on every event is one lookup, and is rebuilt from the graph at start.

use crate::db::graph::Query;
use crate::db::kv::{RedisError, RedisResult};
use crate::db::{exec_single_row, fetch_key_from_graph, get_redis_conn};
use crate::models::error::ModelResult;

/// Redis set of every address the moderator took down.
const TAKEDOWNS_KEY: &str = "Moderation:Removed";

/// Remembers that the moderator took down `uri`.
pub async fn record_takedown(uri: &str) -> ModelResult<()> {
    let query = Query::new(
        "record_takedown",
        "MERGE (t:Takedown {uri: $uri})
         ON CREATE SET t.indexed_at = timestamp()",
    )
    .param("uri", uri.to_string());
    exec_single_row(query).await?;

    let mut redis_conn = get_redis_conn().await?;
    let () = redis::cmd("SADD")
        .arg(TAKEDOWNS_KEY)
        .arg(uri)
        .query_async(&mut redis_conn)
        .await
        .map_err(RedisError::from)?;
    Ok(())
}

/// Whether the moderator took down `uri`. One Redis lookup.
pub async fn is_taken_down(uri: &str) -> RedisResult<bool> {
    let mut redis_conn = get_redis_conn().await?;
    let is_member: bool = redis::cmd("SISMEMBER")
        .arg(TAKEDOWNS_KEY)
        .arg(uri)
        .query_async(&mut redis_conn)
        .await?;
    Ok(is_member)
}

/// Rebuilds the Redis set from the graph, so a Redis wipe or restore forgets no
/// takedown. Returns how many the graph holds.
pub async fn rebuild_takedown_index() -> ModelResult<usize> {
    let query = Query::new(
        "get_takedowns",
        "MATCH (t:Takedown) RETURN collect(t.uri) AS uris",
    );
    let uris: Vec<String> = fetch_key_from_graph(query, "uris")
        .await?
        .unwrap_or_default();

    let mut pipe = redis::pipe();
    pipe.atomic().del(TAKEDOWNS_KEY).ignore();
    for chunk in uris.chunks(1_000) {
        pipe.sadd(TAKEDOWNS_KEY, chunk).ignore();
    }
    let mut redis_conn = get_redis_conn().await?;
    let () = pipe
        .query_async(&mut redis_conn)
        .await
        .map_err(RedisError::from)?;
    Ok(uris.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DynError;
    use crate::{StackConfig, StackManager};

    /// A takedown survives losing Redis: it is rebuilt from the graph.
    #[tokio_shared_rt::test(shared)]
    async fn takedowns_are_remembered_and_rebuilt_from_the_graph() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let uri = format!(
            "pubky://{}/pub/pubky.app/posts/0034FJVRPG4V0",
            pubky::Keypair::random().public_key().to_z32()
        );

        assert!(!is_taken_down(&uri).await?);
        record_takedown(&uri).await?;
        assert!(is_taken_down(&uri).await?);

        let mut redis_conn = get_redis_conn().await?;
        let () = redis::cmd("SREM")
            .arg(TAKEDOWNS_KEY)
            .arg(&uri)
            .query_async(&mut redis_conn)
            .await?;
        assert!(!is_taken_down(&uri).await?, "lost from Redis");

        rebuild_takedown_index().await?;
        assert!(is_taken_down(&uri).await?, "rebuilt from the graph");
        Ok(())
    }
}
