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
use tokio::sync::Mutex as AsyncMutex;

use crate::db::kv::{RedisError, RedisResult};
use crate::db::{fetch_key_from_graph, fetch_row_from_graph, get_redis_conn, queries, RedisOps};
use crate::models::error::{ModelError, ModelResult};
use crate::models::user::{UserCounts, UserDetails, UsersByTagSearch};

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
/// Taken by an [`is_stub`] that finds `Scope:Stubs` lost; per process.
static REBUILD_LOCK: AsyncMutex<()> = AsyncMutex::const_new(());

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
    if let Some(stub) = check(user_id).await? {
        return Ok(stub);
    }
    // Callers that find the set lost together share one rebuild. The lock is taken
    // holding no connection, so waiting callers never keep the pool from the rebuild.
    let _rebuild = REBUILD_LOCK.lock().await;
    if let Some(stub) = check(user_id).await? {
        return Ok(stub);
    }
    rebuild().await?;
    let mut conn = get_redis_conn().await?;
    Ok(conn.sismember(STUBS_KEY, user_id).await?)
}

/// Whether the user is a stub, or `None` if `Scope:Stubs` isn't built.
async fn check(user_id: &str) -> RedisResult<Option<bool>> {
    let mut conn = get_redis_conn().await?;
    let found: Vec<bool> = conn.smismember(STUBS_KEY, &[user_id, BUILT]).await?;
    Ok(match found.as_slice() {
        [stub, true] => Some(*stub),
        _ => None,
    })
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
///
/// A node another caller created first is left as they made it: the watcher and the
/// API can both meet an unknown user, and only the watcher keeps the moderator in
/// scope. Its counts are never overwritten, and its entry stays only if that node is a
/// stub too.
pub async fn create_stub(details: &UserDetails) -> ModelResult<()> {
    let user_id = details.id.to_string();
    write_entry(&user_id, true).await?;
    // Before the node: a follow or tag of it, once it exists, has counts to update.
    init_counts(&user_id).await?;
    let row = fetch_row_from_graph(queries::put::create_stub_user(details)?)
        .await?
        .ok_or_else(|| ModelError::from_generic("creating a stub returned no row"))?;
    let created: bool = row.get("created")?;
    let scope_state: Option<String> = row.get("scope_state")?;
    if !created {
        if scope_state.as_deref() != Some(SCOPE_STATE_STUB) {
            drop_stub_entry(&user_id).await?;
        }
        return Ok(());
    }
    write_entry(&user_id, true).await?;
    Ok(())
}

/// Saves zeroed counts for the user, unless it has counts already: those of a user
/// created first, or of a stub already updated, are never overwritten.
async fn init_counts(user_id: &str) -> RedisResult<()> {
    let key = format!("{}:{user_id}", UserCounts::prefix().await);
    let zeroed = serde_json::to_string(&UserCounts::default())
        .map_err(|e| RedisError::SerializationFailed(Box::new(e)))?;
    let mut conn = get_redis_conn().await?;
    let _: Option<String> = redis::cmd("JSON.SET")
        .arg(key)
        .arg("$")
        .arg(zeroed)
        .arg("NX")
        .query_async(&mut conn)
        .await?;
    Ok(())
}

/// Stops treating the user as a stub in Redis, and ranks it in the users-by-tag search
/// its tags were kept out of. Returns whether it was a stub there. The caller clears its
/// `scope_state`, then calls [`drop_stub_entry`]; the rankings, rebuilt from the graph,
/// take the user in from their next rebuild.
pub async fn unstub(user_id: &str) -> ModelResult<bool> {
    // Out of the set first: the tag search skips a stub.
    if !write_entry(user_id, false).await? {
        return Ok(false);
    }
    if let Err(e) = UsersByTagSearch::sync_user_labels(user_id).await {
        // Back in the set, so the next promotion retries the whole of it.
        write_entry(user_id, true).await?;
        return Err(e.into());
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

    /// A stub written for a user another caller created first, in scope (the watcher's
    /// moderator, met by the API at the same time), leaves that user as it was.
    #[tokio_shared_rt::test(shared)]
    async fn a_stub_write_leaves_a_user_created_first_in_scope() -> Result<(), DynError> {
        use crate::models::traits::Collection;

        StackManager::setup(&StackConfig::default()).await?;
        let user_id = pubky::Keypair::random().public_key().to_z32();
        let pubky_id = pubky_app_specs::PubkyId::try_from(user_id.as_str())?;
        let mut member = crate::models::user::UserDetails::from_pubky(pubky_id.clone());
        member.name = "Moderator".to_string();
        member.put_to_graph().await?;

        let stubbed =
            super::create_stub(&crate::models::user::UserDetails::from_pubky(pubky_id)).await;
        let details = crate::models::user::UserDetails::get_by_id(&user_id).await;
        let state = crate::db::fetch_key_from_graph::<Option<String>>(
            queries::get::get_user_scope_state(&user_id),
            "scope_state",
        )
        .await;
        let stub = super::is_stub(&user_id).await;

        let delete = Query::new(
            "test_delete_member",
            "MATCH (u:User {id: $id}) DETACH DELETE u",
        )
        .param("id", user_id.clone());
        exec_single_row(delete).await?;

        stubbed?;
        assert_eq!(state?.flatten(), None, "still in scope in the graph");
        assert!(!stub?, "and in Redis");
        assert_eq!(
            details?.map(|d| d.name).as_deref(),
            Some("Moderator"),
            "profile kept"
        );
        Ok(())
    }

    /// Counts a stub already has, from an update that landed before the stub was
    /// written, are kept: a new stub's zeroed counts never replace them.
    #[tokio_shared_rt::test(shared)]
    async fn a_new_stub_keeps_counts_already_saved() -> Result<(), DynError> {
        use crate::db::kv::RedisOps;
        use crate::models::user::UserCounts;

        StackManager::setup(&StackConfig::default()).await?;
        let user_id = pubky::Keypair::random().public_key().to_z32();
        let followed = UserCounts {
            followers: 1,
            ..Default::default()
        };
        followed.put_index_json(&[&user_id], None, None).await?;

        let details = crate::models::user::UserDetails::from_pubky(
            pubky_app_specs::PubkyId::try_from(user_id.as_str())?,
        );
        let stubbed = super::create_stub(&details).await;
        let counts = UserCounts::get_from_index(&user_id).await;

        let delete = Query::new(
            "test_delete_stub_user",
            "MATCH (u:User {id: $id}) DETACH DELETE u",
        )
        .param("id", user_id.clone());
        exec_single_row(delete).await?;
        super::drop_stub_entry(&user_id).await?;

        stubbed?;
        assert_eq!(counts?.map(|c| c.followers), Some(1), "the follow is kept");
        Ok(())
    }

    /// Many callers finding the set lost at once all get their answer: they share one
    /// rebuild and none waits on the pool holding a connection the rebuild needs.
    #[tokio_shared_rt::test(shared)]
    async fn many_reads_of_a_lost_stub_set_finish() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let () = deadpool_redis::redis::cmd("DEL")
            .arg(super::STUBS_KEY)
            .query_async(&mut crate::db::get_redis_conn().await?)
            .await
            .map_err(crate::db::kv::RedisError::from)?;

        let mut reads = tokio::task::JoinSet::new();
        for _ in 0..1_000 {
            let user_id = pubky::Keypair::random().public_key().to_z32();
            reads.spawn(async move { super::is_stub(&user_id).await });
        }
        let finished = tokio::time::timeout(std::time::Duration::from_secs(60), async {
            while let Some(read) = reads.join_next().await {
                assert!(!read??, "an unknown user is no stub");
            }
            Ok::<_, DynError>(())
        })
        .await;

        assert!(finished.is_ok(), "the reads stalled on the Redis pool");
        finished?
    }

    /// A rebuild doesn't overwrite a stub written after it read the graph: its
    /// replacement is refused, so the next rebuild reads the graph again.
    ///
    /// The script runs on keys of this test's own: tests running alongside rebuild the
    /// shared `Scope:Stubs` from the graph, which would drop a stub written to Redis only.
    #[tokio_shared_rt::test(shared)]
    async fn a_rebuild_keeps_a_stub_written_after_its_graph_read() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let user_id = pubky::Keypair::random().public_key().to_z32();
        let stubs_key = format!("{}:test:{user_id}", super::STUBS_KEY);
        let writes_key = format!("{}:test:{user_id}", super::WRITES_KEY);
        let mut conn = crate::db::get_redis_conn().await?;

        // A stub write counts itself, so a rebuild that read the graph before it can tell.
        let before: Option<i64> = conn.get(super::WRITES_KEY).await?;
        super::write_entry(&user_id, true).await?;
        let after: Option<i64> = conn.get(super::WRITES_KEY).await?;
        super::write_entry(&user_id, false).await?;

        // The graph is read with no writes counted, then a stub is written.
        let () = deadpool_redis::redis::pipe()
            .atomic()
            .sadd(&stubs_key, &user_id)
            .ignore()
            .incr(&writes_key, 1)
            .ignore()
            .query_async(&mut conn)
            .await
            .map_err(crate::db::kv::RedisError::from)?;
        let replaced: i64 = super::REPLACE_IF_UNCHANGED
            .key(&stubs_key)
            .key(&writes_key)
            .arg("0")
            .arg(super::BUILT)
            .invoke_async(&mut conn)
            .await?;
        let kept: bool = conn.sismember(&stubs_key, &user_id).await?;
        let () = conn.del(&[&stubs_key, &writes_key]).await?;

        assert!(after > before, "the stub write is counted");
        assert_eq!(replaced, 0, "the replacement is refused");
        assert!(kept, "the stub written after the graph read stays");
        Ok(())
    }

    /// The users-by-tag search and its reindex leave a tagged stub out, as the rankings
    /// do; a member tagged the same way is in.
    #[tokio_shared_rt::test(shared)]
    async fn the_users_by_tag_queries_leave_stubs_out() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let [tagger, member, stub] =
            [(); 3].map(|_| pubky::Keypair::random().public_key().to_z32());
        let label = format!("stubtest{}", rand_suffix());
        let create = Query::new(
            "test_tag_a_member_and_a_stub",
            "CREATE (t:User {id: $tagger}), (m:User {id: $member}),
                    (s:User {id: $stub, scope_state: $stub_state}),
                    (t)-[:TAGGED {label: $label}]->(m), (t)-[:TAGGED {label: $label}]->(s)",
        )
        .param("tagger", tagger.clone())
        .param("member", member.clone())
        .param("stub", stub.clone())
        .param("stub_state", SCOPE_STATE_STUB)
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
        .param("ids", vec![tagger, member.clone(), stub]);
        exec_single_row(delete).await?;

        assert_eq!(
            searched?,
            vec![member.clone()],
            "search: the member, not the stub"
        );
        assert_eq!(
            reindexed?,
            vec![member],
            "reindex: the member, not the stub"
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
