//! Outsiders: users this Nexus knows but keeps outside its scope.
//!
//! An outsider's node exists so in-scope records (a follow, a tag, a reply) can point at it,
//! but its own content is never read: it has no `HOSTED_BY` and no cursor. It has
//! counts, which in-scope records move, but is kept out of the rankings and its posts
//! out of the global feeds, where a profile never read would show as a blank card.
//!
//! The graph's `scope_state` is the one record of who is an outsider: every check reads it.

use crate::db::kv::{RedisError, RedisResult};
use crate::db::{exec_single_row, fetch_key_from_graph, queries};
use crate::models::error::ModelResult;
use crate::models::user::{UserDetails, UsersByTagSearch};

/// `scope_state` of a user known to this Nexus but outside its scope.
pub const SCOPE_STATE_OUTSIDE: &str = "outside";
/// `scope_state` of a member that left the scope: no longer polled, content kept.
pub const SCOPE_STATE_LEFT: &str = "left";

/// Whether the user is an outsider, by the graph's `scope_state` (an indexed lookup).
pub async fn is_outsider(user_id: &str) -> RedisResult<bool> {
    let query = queries::get::get_user_scope_state(user_id);
    let scope_state: Option<Option<String>> = fetch_key_from_graph(query, "scope_state")
        .await
        .map_err(|e| RedisError::CommandFailed(format!("reading the scope state: {e}").into()))?;
    Ok(scope_state.flatten().as_deref() == Some(SCOPE_STATE_OUTSIDE))
}

/// Creates the node of a user met outside the scope, already an outsider.
///
/// The node and its `scope_state` are written in one graph write, so no node exists
/// without its state for the HS resolver to bind.
///
/// A node another caller created first is left as they made it: the watcher and the
/// API can both meet an unknown user, and only the watcher keeps the moderator in
/// scope.
pub async fn create_outsider(details: &UserDetails) -> ModelResult<()> {
    exec_single_row(queries::put::create_outsider_user(details)?).await?;
    Ok(())
}

/// Lets an outsider into the scope: clears its `scope_state`, and ranks it in the
/// users-by-tag search its tags were kept out of; the rankings, rebuilt from the graph,
/// take it in from their next rebuild. Returns whether it was an outsider.
///
/// If the ranking fails, it is an outsider again, so a retry lets it in from the start.
pub async fn admit_outsider(user_id: &str) -> ModelResult<bool> {
    if !is_outsider(user_id).await? {
        return Ok(false);
    }
    // Cleared first: the tag search skips an outsider.
    exec_single_row(queries::put::set_user_scope_state(user_id, None)).await?;
    if let Err(e) = UsersByTagSearch::sync_user_labels(user_id).await {
        let query = queries::put::set_user_scope_state(user_id, Some(SCOPE_STATE_OUTSIDE));
        exec_single_row(query).await?;
        return Err(e.into());
    }
    Ok(true)
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

    use super::SCOPE_STATE_OUTSIDE;

    /// The all-posts feed served from the graph (a kind filter) leaves an outsider's posts
    /// out, as the feed served from Redis does; its author coming into scope lets them
    /// back in.
    #[tokio_shared_rt::test(shared)]
    async fn the_graph_served_feed_leaves_outsiders_posts_out() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let author = pubky::Keypair::random().public_key().to_z32();
        // Far in the future, so the window holds this post alone.
        let indexed_at = 4_000_000_000_000_i64 + rand_suffix();
        let post_id = format!("OUTSIDE{indexed_at}");
        let create = Query::new(
            "test_outsider_author_with_post",
            "CREATE (u:User {id: $author, trust: 1.0, scope_state: $outsider})
                 -[:AUTHORED]->(:Post {id: $post_id, kind: 'short', indexed_at: $at})",
        )
        .param("author", author.clone())
        .param("outsider", SCOPE_STATE_OUTSIDE)
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

        let while_outsider = in_feed().await;
        let cleared = exec_single_row(queries::put::set_user_scope_state(&author, None)).await;
        let in_scope = in_feed().await;

        let delete = Query::new(
            "test_delete_outsider_author",
            "MATCH (u:User {id: $author}) OPTIONAL MATCH (u)-[:AUTHORED]->(p:Post) DETACH DELETE u, p",
        )
        .param("author", author);
        exec_single_row(delete).await?;

        cleared?;
        assert!(
            !while_outsider?,
            "an outsider's post stays out of the shared feed"
        );
        assert!(in_scope?, "control: in scope, the same post is served");
        Ok(())
    }

    /// An outsider written for a user another caller created first, in scope (the watcher's
    /// moderator, met by the API at the same time), leaves that user as it was.
    #[tokio_shared_rt::test(shared)]
    async fn an_outsider_write_leaves_a_user_created_first_in_scope() -> Result<(), DynError> {
        use crate::models::traits::Collection;

        StackManager::setup(&StackConfig::default()).await?;
        let user_id = pubky::Keypair::random().public_key().to_z32();
        let pubky_id = pubky_app_specs::PubkyId::try_from(user_id.as_str())?;
        let mut member = crate::models::user::UserDetails::from_pubky(pubky_id.clone());
        member.name = "Moderator".to_string();
        member.put_to_graph().await?;

        let kept_out =
            super::create_outsider(&crate::models::user::UserDetails::from_pubky(pubky_id)).await;
        let details = crate::models::user::UserDetails::get_by_id(&user_id).await;
        let state = crate::db::fetch_key_from_graph::<Option<String>>(
            queries::get::get_user_scope_state(&user_id),
            "scope_state",
        )
        .await;
        let outsider = super::is_outsider(&user_id).await;

        let delete = Query::new(
            "test_delete_member",
            "MATCH (u:User {id: $id}) DETACH DELETE u",
        )
        .param("id", user_id.clone());
        exec_single_row(delete).await?;

        kept_out?;
        assert_eq!(state?.flatten(), None, "still in scope in the graph");
        assert!(!outsider?, "and not taken for an outsider");
        assert_eq!(
            details?.map(|d| d.name).as_deref(),
            Some("Moderator"),
            "profile kept"
        );
        Ok(())
    }

    /// The users-by-tag search and its reindex leave a tagged outsider out, as the rankings
    /// do; a member tagged the same way is in.
    #[tokio_shared_rt::test(shared)]
    async fn the_users_by_tag_queries_leave_outsiders_out() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let [tagger, member, outsider] =
            [(); 3].map(|_| pubky::Keypair::random().public_key().to_z32());
        let label = format!("outsidertest{}", rand_suffix());
        let create = Query::new(
            "test_tag_a_member_and_a_outsider",
            "CREATE (t:User {id: $tagger}), (m:User {id: $member}),
                    (s:User {id: $outsider, scope_state: $outsider_state}),
                    (t)-[:TAGGED {label: $label}]->(m), (t)-[:TAGGED {label: $label}]->(s)",
        )
        .param("tagger", tagger.clone())
        .param("member", member.clone())
        .param("outsider", outsider.clone())
        .param("outsider_state", SCOPE_STATE_OUTSIDE)
        .param("label", label.clone());
        exec_single_row(create).await?;

        let searched =
            async {
                let rows = crate::db::fetch_all_rows_from_graph(
                    queries::get::search_users_by_tags(std::slice::from_ref(&label), None, None),
                )
                .await?;
                rows.iter()
                    .map(|row| row.get::<String>("user_id"))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(DynError::from)
            }
            .await;
        let reindexed = async {
            let rows =
                crate::db::fetch_all_rows_from_graph(queries::get::global_tags_by_user()).await?;
            let mut users = Vec::new();
            for row in &rows {
                if row.get::<String>("label")? == label {
                    let sorted_set: Vec<(f64, String)> = row.get("sorted_set")?;
                    users.extend(sorted_set.into_iter().map(|(_, id)| id));
                }
            }
            Ok::<_, DynError>(users)
        }
        .await;

        let delete = Query::new(
            "test_delete_tagged_users",
            "MATCH (u:User) WHERE u.id IN $ids DETACH DELETE u",
        )
        .param("ids", vec![tagger, member.clone(), outsider]);
        exec_single_row(delete).await?;

        assert_eq!(
            searched?,
            vec![member.clone()],
            "search: the member, not the outsider"
        );
        assert_eq!(
            reindexed?,
            vec![member],
            "reindex: the member, not the outsider"
        );
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
