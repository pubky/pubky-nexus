//! Posts, tags and files the moderator took down, so they stay down whatever order
//! events arrive in.
//!
//! The moderator's tag deletes its target when read. If the target isn't indexed yet
//! (a new Nexus reads everyone's history in no particular order), there is nothing to
//! delete, and the target would land when its own event is read later; remembering the
//! address drops that event. Profiles aren't remembered: a moderated user may come back
//! with a new profile at the same address.
//!
//! The graph (`:Takedown` nodes) is the source of truth; `Moderation:Removed` mirrors it
//! for the per-event check and is rebuilt from the graph at start.

use crate::db::kv::{sets, RedisResult};
use crate::db::{exec_single_row, fetch_key_from_graph, queries};
use crate::models::error::ModelResult;

const TAKEDOWNS: (&str, &str) = ("Moderation", "Removed");

/// Remembers that the moderator took down `uri`.
pub async fn record_takedown(uri: &str) -> ModelResult<()> {
    exec_single_row(queries::put::record_takedown(uri)).await?;
    sets::put(TAKEDOWNS.0, TAKEDOWNS.1, &[uri], None).await?;
    Ok(())
}

pub async fn is_taken_down(uri: &str) -> RedisResult<bool> {
    sets::is_member(TAKEDOWNS.0, TAKEDOWNS.1, uri).await
}

/// Rebuilds `Moderation:Removed` from the graph. Returns how many takedowns it holds.
pub async fn rebuild_takedown_index() -> ModelResult<usize> {
    let uris: Vec<String> = fetch_key_from_graph(queries::get::get_takedown_uris(), "uris")
        .await?
        .unwrap_or_default();
    sets::replace(TAKEDOWNS.0, TAKEDOWNS.1, &uris).await?;
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

        sets::del(TAKEDOWNS.0, TAKEDOWNS.1, &[&uri]).await?;
        assert!(!is_taken_down(&uri).await?, "lost from Redis");

        rebuild_takedown_index().await?;
        assert!(is_taken_down(&uri).await?, "rebuilt from the graph");
        Ok(())
    }
}
