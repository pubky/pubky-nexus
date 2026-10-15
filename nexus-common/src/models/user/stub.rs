//! Stubs: users this Nexus knows but keeps outside its scope.
//!
//! A stub's node exists so in-scope records (a follow, a tag, a reply) can point at it,
//! but its own content is never read: it has no `HOSTED_BY` and no cursor. It has
//! counts, which in-scope records move, but is kept out of the rankings and its posts
//! out of the global feeds, where a profile never read would show as a blank card.
//!
//! The graph's `scope_state` is the source of truth; `Scope:Stubs` mirrors it for the
//! per-write check and is rebuilt from the graph at start.

use crate::db::kv::{sets, RedisResult};
use crate::db::{exec_single_row, fetch_key_from_graph, queries, RedisOps};
use crate::models::error::ModelResult;
use crate::models::post::trust_filter;
use crate::models::user::{UserCounts, SCOPE_STATE_LEFT};

/// `scope_state` of a user known to this Nexus but outside its scope.
pub const SCOPE_STATE_STUB: &str = "stub";
const STUBS: (&str, &str) = ("Scope", "Stubs");

pub async fn is_stub(user_id: &str) -> RedisResult<bool> {
    sets::is_member(STUBS.0, STUBS.1, user_id).await
}

/// Whether nothing reads the user's stream: a stub, or a former member that left the
/// scope. A post of theirs that an in-scope record needs has to be fetched for it.
pub async fn is_unread(user_id: &str) -> ModelResult<bool> {
    if is_stub(user_id).await? {
        return Ok(true);
    }
    let query = queries::get::get_user_scope_state(user_id);
    let scope_state: Option<Option<String>> = fetch_key_from_graph(query, "scope_state").await?;
    Ok(scope_state.flatten().as_deref() == Some(SCOPE_STATE_LEFT))
}

/// Makes an existing user node a stub, with zeroed counts outside the rankings.
pub async fn mark_stub(user_id: &str) -> ModelResult<()> {
    let query = queries::put::set_user_scope_state(user_id, Some(SCOPE_STATE_STUB));
    exec_single_row(query).await?;
    sets::put(STUBS.0, STUBS.1, &[user_id], None).await?;
    UserCounts::default()
        .put_index_json(&[user_id], None, None)
        .await?;
    Ok(())
}

/// Stops treating the user as a stub in Redis, puts its counts into the rankings
/// and its posts into the global feeds.
/// Returns whether it was a stub. The caller clears its `scope_state`.
pub async fn unstub(user_id: &str) -> ModelResult<bool> {
    if !sets::take(STUBS.0, STUBS.1, user_id).await? {
        return Ok(false);
    }
    if let Some(counts) = UserCounts::get_from_index(user_id).await? {
        counts.put_to_index(user_id).await?;
    }
    trust_filter::show_author(user_id).await?;
    Ok(true)
}

/// Rebuilds `Scope:Stubs` from the graph. Returns how many stubs the graph holds.
pub async fn rebuild_stub_index() -> ModelResult<usize> {
    let query = queries::get::get_user_ids_by_scope_state(SCOPE_STATE_STUB);
    let stubs: Vec<String> = fetch_key_from_graph(query, "user_ids")
        .await?
        .unwrap_or_default();
    sets::replace(STUBS.0, STUBS.1, &stubs).await?;
    Ok(stubs.len())
}

#[cfg(test)]
mod tests {
    use crate::db::graph::Query;
    use crate::db::kv::SortOrder;
    use crate::db::{exec_single_row, queries};
    use crate::models::post::{KindFilter, PostStream, StreamSource};
    use crate::types::{DynError, Pagination, StreamSorting};
    use crate::{StackConfig, StackManager};
    use pubky_app_specs::PubkyAppPostKind;

    use super::SCOPE_STATE_STUB;

    /// The all-posts feed served from the graph (a kind filter) leaves a stub's posts
    /// out, as the feed served from Redis does; its author coming into scope lets them
    /// back in.
    #[tokio_shared_rt::test(shared)]
    async fn the_graph_served_feed_leaves_stubs_posts_out() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let author = pubky::Keypair::random().public_key().to_z32();
        // Far in the future, so the window holds this post alone.
        let indexed_at = 4_000_000_000_000_i64 + rand_suffix();
        let post_id = format!("STUB{indexed_at}");
        let create = Query::new(
            "test_stub_author_with_post",
            "CREATE (u:User {id: $author, trust: 1.0, scope_state: $stub})
                 -[:AUTHORED]->(:Post {id: $post_id, kind: 'short', indexed_at: $at})",
        )
        .param("author", author.clone())
        .param("stub", SCOPE_STATE_STUB)
        .param("post_id", post_id.clone())
        .param("at", indexed_at);
        exec_single_row(create).await?;

        let in_feed = || async {
            let pagination = Pagination {
                // Not start == end: Neo4j then seeks the bound itself and returns the
                // float bound in place of the stored integer.
                start: Some(indexed_at as f64),
                end: Some((indexed_at - 1) as f64),
                limit: Some(10),
                ..Default::default()
            };
            let stream = PostStream::get_post_keys(
                StreamSource::All,
                pagination,
                SortOrder::Descending,
                StreamSorting::Timeline,
                None,
                None,
                Some(KindFilter::Kind(PubkyAppPostKind::Short)),
            )
            .await?;
            let keys = stream.map(|stream| stream.post_keys).unwrap_or_default();
            Ok::<_, DynError>(keys.contains(&format!("{author}:{post_id}")))
        };

        let while_stub = in_feed().await;
        let cleared = exec_single_row(queries::put::set_user_scope_state(&author, None)).await;
        let in_scope = in_feed().await;

        let delete = Query::new(
            "test_delete_stub_author",
            "MATCH (u:User {id: $author}) OPTIONAL MATCH (u)-[:AUTHORED]->(p:Post) DETACH DELETE u, p",
        )
        .param("author", author);
        exec_single_row(delete).await?;

        cleared?;
        assert!(!while_stub?, "a stub's post stays out of the shared feed");
        assert!(in_scope?, "control: in scope, the same post is served");
        Ok(())
    }

    /// Keeps parallel runs of the test in distinct windows.
    fn rand_suffix() -> i64 {
        (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
            % 1_000_000) as i64
    }
}
