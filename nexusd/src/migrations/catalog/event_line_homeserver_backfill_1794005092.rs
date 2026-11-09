use crate::migrations::manager::Migration;
use async_trait::async_trait;
use nexus_common::db::graph::Query;
use nexus_common::db::{fetch_all_rows_from_graph, get_redis_conn, RedisOps};
use nexus_common::models::event::EventLine;
use nexus_common::types::DynError;
use pubky_app_specs::PubkyId;
use redis::Script;
use std::collections::{BTreeSet, HashMap};
use tracing::info;

/// Give the event lines stored before Nexus kept the homeserver key that key, where the graph
/// knows it.
///
/// # What this does
/// Walks the `Events` list one page at a time. For each line without a key it takes the user
/// from the line's address and appends the key of the homeserver the user's `HOSTED_BY` edge
/// points to. A line whose user has no edge, or whose user can't be read, is left as it is,
/// without a key.
///
/// The edge says where the user is hosted now, not where an old event was read from. For a
/// user who moved, their older lines get the new homeserver.
///
/// # Batching
/// Each page's users are looked up in one query, and its lines are rewritten in place by one
/// Lua script. The list is never rebuilt, so the `/v0/events` cursors, which are offsets into
/// it, stay valid while this runs.
///
/// # Idempotency
/// A line that already ends with a key is left alone, and the script writes a line only while
/// it still holds what was read, so a failed run is safe to repeat. A repeated run also gives a
/// key to the lines of users who have gained an edge since.
///
/// # Deploy ordering
/// Run it after the watchers that store the key are deployed. A line an older watcher appends
/// after the walk passed its end stays without a key.
pub struct EventLineHomeserverBackfill1794005092;

/// Event lines read and rewritten per page.
const EVENT_PAGE_SIZE: usize = 500;

/// Whether `line` already ends with a homeserver key: a public key after its last space, with
/// the event type and the address before it.
fn has_homeserver(line: &str) -> bool {
    line.rsplit_once(' ')
        .is_some_and(|(head, last)| head.contains(' ') && PubkyId::try_from(last).is_ok())
}

/// The user in the address of a `METHOD pubky://<user_id>/<path>` line.
fn line_user_id(line: &str) -> Option<PubkyId> {
    let (_, uri) = line.split_once(' ')?;
    let (user_id, _) = uri.strip_prefix("pubky://")?.split_once('/')?;
    PubkyId::try_from(user_id).ok()
}

/// The homeserver each of `user_ids` has a `HOSTED_BY` edge to. Users without one are absent.
async fn hosting_homeservers(user_ids: Vec<String>) -> Result<HashMap<String, String>, DynError> {
    let query = Query::new(
        "event_line_homeserver_backfill_hosted_by",
        "UNWIND $user_ids AS user_id
         MATCH (:User {id: user_id})-[:HOSTED_BY]->(hs:Homeserver)
         RETURN user_id, head(collect(hs.id)) AS hs_id",
    )
    .param("user_ids", user_ids);

    let mut homeservers = HashMap::new();
    for row in fetch_all_rows_from_graph(query).await? {
        homeservers.insert(row.get("user_id")?, row.get("hs_id")?);
    }
    Ok(homeservers)
}

/// Sets each `(index, read, rewritten)` entry of the list at `key` to `rewritten` while it still
/// holds `read`, and returns how many were set.
async fn rewrite_lines(key: &str, lines: &[(u64, String, String)]) -> Result<usize, DynError> {
    let script = Script::new(
        r#"
        local set = 0
        for i = 1, #ARGV, 3 do
            local index = tonumber(ARGV[i])
            if redis.call('LINDEX', KEYS[1], index) == ARGV[i + 1] then
                redis.call('LSET', KEYS[1], index, ARGV[i + 2])
                set = set + 1
            end
        end
        return set
    "#,
    );

    let mut invocation = script.prepare_invoke();
    invocation.key(key);
    for (index, read, rewritten) in lines {
        invocation.arg(index).arg(read).arg(rewritten);
    }

    let mut conn = get_redis_conn().await?;
    Ok(invocation.invoke_async(&mut conn).await?)
}

#[async_trait]
impl Migration for EventLineHomeserverBackfill1794005092 {
    fn id(&self) -> &'static str {
        "EventLineHomeserverBackfill1794005092"
    }

    fn is_multi_staged(&self) -> bool {
        false
    }

    async fn dual_write(_data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        let key = format!("{}:Events", EventLine::prefix().await);

        let mut cursor: u64 = 0;
        let mut lines_read: u64 = 0;
        let mut lines_rewritten: usize = 0;
        let mut lines_left: usize = 0;
        loop {
            let (lines, next_cursor) =
                EventLine::get_from_index(Some(cursor), EVENT_PAGE_SIZE).await?;
            // Only an empty page is the end: lines appended while this runs are walked too.
            if lines.is_empty() {
                break;
            }

            let keyless: Vec<(u64, String)> = (cursor..)
                .zip(lines.iter())
                .filter(|(_, line)| !has_homeserver(line))
                .map(|(index, line)| (index, line.clone()))
                .collect();
            lines_read += lines.len() as u64;
            cursor = next_cursor;
            if keyless.is_empty() {
                continue;
            }

            let user_ids: BTreeSet<String> = keyless
                .iter()
                .filter_map(|(_, line)| line_user_id(line))
                .map(|user_id| user_id.to_string())
                .collect();
            let homeservers = hosting_homeservers(user_ids.into_iter().collect()).await?;

            let keyless_count = keyless.len();
            let rewrites: Vec<(u64, String, String)> = keyless
                .into_iter()
                .filter_map(|(index, line)| {
                    let homeserver_id = homeservers.get(line_user_id(&line)?.as_ref())?;
                    let rewritten = EventLine::new(&line, homeserver_id).as_str().to_string();
                    Some((index, line, rewritten))
                })
                .collect();
            lines_left += keyless_count - rewrites.len();
            if !rewrites.is_empty() {
                lines_rewritten += rewrite_lines(&key, &rewrites).await?;
            }

            info!(
                "EventLineHomeserverBackfill migration: {lines_read} event lines read, \
                 {lines_rewritten} given a homeserver, {lines_left} left without one"
            );
        }

        info!(
            "EventLineHomeserverBackfill migration: complete — {lines_read} event lines read, \
             {lines_rewritten} given a homeserver, {lines_left} left without one"
        );
        Ok(())
    }

    async fn cutover(&self) -> Result<(), DynError> {
        Ok(())
    }

    async fn cleanup(&self) -> Result<(), DynError> {
        Ok(())
    }
}
