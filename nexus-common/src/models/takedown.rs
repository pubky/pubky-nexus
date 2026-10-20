//! Posts, tags and files the moderator took down, so they stay down whatever order
//! events arrive in.
//!
//! The moderator's tag deletes its target when read. If the target isn't indexed yet
//! (a new Nexus reads everyone's history in no particular order), there is nothing to
//! delete, and the target would land when its own event is read later; remembering the
//! address drops that event. Profiles aren't remembered: a moderated user may come back
//! with a new profile at the same address.
//!
//! Each takedown is a `:Takedown {uri, tag_uri}` node: the canonical address taken down,
//! and the address of the moderator tag that took it down. A takedown is permanent for
//! now: lifting one is not implemented. `tag_uri` is kept so it can be, by matching the
//! DEL of that moderator tag to its takedown.

use crate::db::{exec_single_row, fetch_key_from_graph, queries};
use crate::models::error::ModelResult;

/// Remembers that the moderator tag at `tag_uri` took down `uri`, a canonical address.
pub async fn record_takedown(uri: &str, tag_uri: &str) -> ModelResult<()> {
    exec_single_row(queries::put::record_takedown(uri, tag_uri)).await?;
    Ok(())
}

/// Whether the moderator took down `uri`, a canonical address.
pub async fn is_taken_down(uri: &str) -> ModelResult<bool> {
    let query = queries::get::is_taken_down(uri);
    Ok(fetch_key_from_graph(query, "taken_down")
        .await?
        .unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::fetch_row_from_graph;
    use crate::db::graph::Query;
    use crate::types::DynError;
    use crate::{StackConfig, StackManager};

    /// A takedown is remembered with the moderator tag that made it.
    #[tokio_shared_rt::test(shared)]
    async fn takedowns_are_remembered_with_their_moderator_tag() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let user = pubky::Keypair::random().public_key().to_z32();
        let uri = format!("pubky://{user}/pub/pubky.app/posts/0034FJVRPG4V0");
        let tag_uri = format!("pubky://{user}/pub/pubky.app/tags/MODERATORTAG");

        assert!(!is_taken_down(&uri).await?);
        record_takedown(&uri, &tag_uri).await?;
        assert!(is_taken_down(&uri).await?);

        let query = Query::new(
            "test_takedown_tag_uri",
            "MATCH (t:Takedown {uri: $uri}) RETURN t.tag_uri AS tag_uri",
        )
        .param("uri", uri.clone());
        let row = fetch_row_from_graph(query).await?.expect("the takedown");
        let stored: String = row.get("tag_uri")?;

        let delete = Query::new(
            "test_delete_takedown",
            "MATCH (t:Takedown {uri: $uri}) DELETE t",
        )
        .param("uri", uri);
        exec_single_row(delete).await?;

        assert_eq!(stored, tag_uri);
        Ok(())
    }
}
