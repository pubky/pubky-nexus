//! Stubs: users this Nexus knows but keeps outside its scope.
//!
//! A stub's node exists so in-scope records (a follow, a tag, a reply) can point at it,
//! but its own content is never read: it has no `HOSTED_BY` and no cursor. It has
//! counts, which in-scope records move, but is kept out of the rankings and its posts
//! out of the global feeds, where a profile never read would show as a blank card.
//!
//! The graph's `scope_state` is the source of truth; `Scope:Stubs` mirrors it for the
//! per-write check and is rebuilt from the graph at start.

use std::sync::LazyLock;

use deadpool_redis::redis::{self, AsyncCommands, Script};

use crate::db::kv::{RedisError, RedisResult};
use crate::db::{exec_single_row, fetch_key_from_graph, get_redis_conn, queries, RedisOps};
use crate::models::error::ModelResult;
use crate::models::user::{UserCounts, UserDetails};

/// `scope_state` of a user known to this Nexus but outside its scope.
pub const SCOPE_STATE_STUB: &str = "stub";
const STUBS_KEY: &str = "Scope:Stubs";
/// Counts the writes to `Scope:Stubs`, so a rebuild can tell whether one landed after it
/// read the graph.
const WRITES_KEY: &str = "Scope:StubWrites";
/// A member every rebuilt `Scope:Stubs` holds (not a valid user id), so a set lost with
/// Redis reads as not built yet rather than as "no stubs".
const BUILT: &str = "*built*";
/// Rebuilds that lost to a stub write and read the graph again, before giving up.
const REBUILD_ATTEMPTS: usize = 5;

/// Replaces `Scope:Stubs` (KEYS[1]) with ARGV[2..], unless the write count (KEYS[2]) is no
/// longer ARGV[1]: a stub written since the graph was read would be lost.
static REPLACE_IF_UNCHANGED: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        "if (redis.call('get', KEYS[2]) or '0') ~= ARGV[1] then return 0 end
         redis.call('del', KEYS[1])
         for i = 2, #ARGV, 1000 do
             redis.call('sadd', KEYS[1], unpack(ARGV, i, math.min(i + 999, #ARGV)))
         end
         return 1",
    )
});

/// Whether the user is a stub. If `Scope:Stubs` was lost (Redis restarted or flushed
/// while the watcher or the API kept running), it is rebuilt from the graph first, so
/// stubs are never taken for members and ranked.
pub async fn is_stub(user_id: &str) -> RedisResult<bool> {
    let mut conn = get_redis_conn().await?;
    let found: Vec<bool> = conn.smismember(STUBS_KEY, &[user_id, BUILT]).await?;
    if found.get(1).copied().unwrap_or(false) {
        return Ok(found.first().copied().unwrap_or(false));
    }
    rebuild().await?;
    Ok(conn.sismember(STUBS_KEY, user_id).await?)
}

/// Adds (`true`) or removes the user's `Scope:Stubs` entry, counting the write in the
/// same step. Returns whether the set changed.
async fn write_entry(user_id: &str, stub: bool) -> RedisResult<bool> {
    let mut conn = get_redis_conn().await?;
    let mut pipe = redis::pipe();
    pipe.atomic();
    match stub {
        true => pipe.sadd(STUBS_KEY, user_id),
        false => pipe.srem(STUBS_KEY, user_id),
    };
    let (changed, _): (usize, i64) = pipe.incr(WRITES_KEY, 1).query_async(&mut conn).await?;
    Ok(changed > 0)
}

/// Creates the node of a user met outside the scope, already a stub, with zeroed
/// counts outside the rankings.
///
/// The node and its `scope_state` are written in one graph write, so no node exists
/// without its state for the HS resolver to bind. Redis is written first, so a failure
/// leaves no node and the retry ingests the user from scratch; and again after the
/// graph write, which also makes a rebuild that read the graph before it read again.
pub async fn create_stub(details: &UserDetails) -> ModelResult<()> {
    let user_id = details.id.to_string();
    write_entry(&user_id, true).await?;
    UserCounts::default()
        .put_index_json(&[&user_id], None, None)
        .await?;
    exec_single_row(queries::put::create_stub_user(details)?).await?;
    write_entry(&user_id, true).await?;
    Ok(())
}

/// Stops treating the user as a stub in Redis and puts its counts into the rankings.
/// Returns whether it was a stub there. The caller clears its `scope_state`, then calls
/// [`drop_stub_entry`].
pub async fn unstub(user_id: &str) -> ModelResult<bool> {
    // Out of the set first: the rankings skip a stub.
    if !write_entry(user_id, false).await? {
        return Ok(false);
    }
    if let Some(counts) = UserCounts::get_from_index(user_id).await? {
        counts.put_to_index(user_id).await?;
    }
    Ok(true)
}

/// Drops the user's `Scope:Stubs` entry, for a user whose graph state was just cleared:
/// a rebuild may have put it back from the state before.
pub async fn drop_stub_entry(user_id: &str) -> ModelResult<()> {
    write_entry(user_id, false).await?;
    Ok(())
}

/// Rebuilds `Scope:Stubs` from the graph. Returns how many stubs the graph holds.
pub async fn rebuild_stub_index() -> ModelResult<usize> {
    Ok(rebuild().await?)
}

/// Replaces `Scope:Stubs` with the graph's stubs, unless a stub write landed after the
/// graph was read; then it reads the graph again.
async fn rebuild() -> RedisResult<usize> {
    for _ in 0..REBUILD_ATTEMPTS {
        let mut conn = get_redis_conn().await?;
        let writes: Option<String> = conn.get(WRITES_KEY).await?;
        let query = queries::get::get_user_ids_by_scope_state(SCOPE_STATE_STUB);
        let stubs: Vec<String> = fetch_key_from_graph(query, "user_ids")
            .await
            .map_err(|e| RedisError::CommandFailed(format!("reading the stubs: {e}").into()))?
            .unwrap_or_default();
        let replaced: i64 = REPLACE_IF_UNCHANGED
            .key(STUBS_KEY)
            .key(WRITES_KEY)
            .arg(writes.as_deref().unwrap_or("0"))
            .arg(&stubs)
            .arg(BUILT)
            .invoke_async(&mut conn)
            .await?;
        if replaced == 1 {
            return Ok(stubs.len());
        }
    }
    Err(RedisError::CommandFailed(
        "Scope:Stubs kept changing while it was rebuilt".into(),
    ))
}

#[cfg(test)]
mod tests {
    use crate::db::graph::Query;
    use crate::db::kv::SortOrder;
    use crate::db::{exec_single_row, queries};
    use crate::models::post::{KindFilter, PostStream, StreamSource};
    use crate::types::{DynError, Pagination, StreamSorting};
    use crate::{StackConfig, StackManager};
    use deadpool_redis::redis::AsyncCommands;
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

    /// A stub stays a stub when Redis loses `Scope:Stubs` while running: the set is
    /// rebuilt from the graph before the answer, never read as "no stubs".
    #[tokio_shared_rt::test(shared)]
    async fn a_lost_stub_set_is_rebuilt_before_it_is_read() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let user_id = pubky::Keypair::random().public_key().to_z32();
        let details = crate::models::user::UserDetails::from_pubky(
            pubky_app_specs::PubkyId::try_from(user_id.as_str())?,
        );
        super::create_stub(&details).await?;
        assert!(super::is_stub(&user_id).await?);

        let () = deadpool_redis::redis::cmd("DEL")
            .arg("Scope:Stubs")
            .query_async(&mut crate::db::get_redis_conn().await?)
            .await
            .map_err(crate::db::kv::RedisError::from)?;
        let after_loss = super::is_stub(&user_id).await;

        let delete = Query::new(
            "test_delete_stub_user",
            "MATCH (u:User {id: $id}) DETACH DELETE u",
        )
        .param("id", user_id.clone());
        exec_single_row(delete).await?;
        super::rebuild_stub_index().await?;

        assert!(after_loss?, "rebuilt from the graph, not read as empty");
        Ok(())
    }

    /// A rebuild doesn't overwrite a stub written after it read the graph: its
    /// replacement is refused, so the next rebuild reads the graph again.
    #[tokio_shared_rt::test(shared)]
    async fn a_rebuild_keeps_a_stub_written_after_its_graph_read() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let user_id = pubky::Keypair::random().public_key().to_z32();
        let mut conn = crate::db::get_redis_conn().await?;
        let read_at: Option<String> = conn.get(super::WRITES_KEY).await?;

        super::write_entry(&user_id, true).await?;
        let replaced: i64 = super::REPLACE_IF_UNCHANGED
            .key(super::STUBS_KEY)
            .key(super::WRITES_KEY)
            .arg(read_at.as_deref().unwrap_or("0"))
            .arg(super::BUILT)
            .invoke_async(&mut conn)
            .await?;
        let kept: bool = conn.sismember(super::STUBS_KEY, &user_id).await?;
        super::write_entry(&user_id, false).await?;

        assert_eq!(replaced, 0, "the replacement is refused");
        assert!(kept, "the stub written after the graph read stays");
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
