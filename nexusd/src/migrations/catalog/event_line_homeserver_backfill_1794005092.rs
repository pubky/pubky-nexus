use crate::migrations::manager::Migration;
use async_trait::async_trait;
use nexus_common::db::graph::Query;
use nexus_common::db::{fetch_all_rows_from_graph, get_redis_conn, RedisOps};
use nexus_common::models::event::EventLine;
use nexus_common::types::DynError;
use pubky_social_specs::PubkyId;
use redis::Script;
use std::collections::{BTreeSet, HashMap};
use tracing::{info, warn};

/// Give the event lines stored before Nexus kept the homeserver key that key, where the graph
/// knows it.
///
/// # What this does
/// Walks the `Events` list one page at a time. For each line without a key, read as
/// [`EventLine::split`] reads it, it takes the user from the line's address and appends the key
/// of the homeserver the user's `HOSTED_BY` edge points to. A line whose user has no edge, or
/// whose user can't be read, is left as it is, without a key.
///
/// The edge names the homeserver the user was bound to, not the one an old event was read
/// from. A user who moves keeps the edge: the resolver marks it stale instead of switching it,
/// and the stale flag is ignored here. So a user who moved after being bound gets the
/// homeserver they were bound to, and only a user first bound after moving gets the new one.
///
/// # Batching
/// Each page's users are looked up in one query, and its lines are rewritten in place by one
/// Lua script. The list is never rebuilt, so the `/v0/events` cursors, which are offsets into
/// it, stay valid while this runs.
///
/// # Idempotency
/// A line that already ends with a key is left alone, and the script writes a line only while
/// it still holds what was read, so a failed run is safe to repeat. `is_multi_staged()` is
/// `false`, so the manager marks this Done after one successful backfill: a line left without a
/// key keeps none after its user gains an edge.
///
/// # Deploy ordering
/// Run it after the watchers that store the key are deployed. A line an older watcher appends
/// after the walk passed its end stays without a key.
pub struct EventLineHomeserverBackfill1794005092;

/// Event lines read and rewritten per page.
const EVENT_PAGE_SIZE: usize = 500;

/// The lines of a page read at `cursor` that have no key, each with its index in the list.
fn keyless_lines(cursor: u64, lines: &[String]) -> Vec<(u64, String)> {
    (cursor..)
        .zip(lines)
        .filter(|(_, line)| EventLine::split(line).1.is_none())
        .map(|(index, line)| (index, line.clone()))
        .collect()
}

/// The user in the address of a stored `METHOD pubky://<user_id>/<path>` line, keyed or not.
fn line_user_id(line: &str) -> Option<PubkyId> {
    let (received, _) = EventLine::split(line);
    let (_, uri) = received.split_once(' ')?;
    let (user_id, _) = uri.strip_prefix("pubky://")?.split_once('/')?;
    PubkyId::try_from(user_id).ok()
}

/// The homeserver each of `user_ids` has a `HOSTED_BY` edge to. Users without one are absent,
/// as is a user whose edge names no public key.
async fn hosting_homeservers(user_ids: Vec<String>) -> Result<HashMap<String, PubkyId>, DynError> {
    let query = Query::new(
        "event_line_homeserver_backfill_hosted_by",
        "UNWIND $user_ids AS user_id
         MATCH (:User {id: user_id})-[:HOSTED_BY]->(hs:Homeserver)
         RETURN user_id, head(collect(hs.id)) AS hs_id",
    )
    .param("user_ids", user_ids);

    let mut homeservers = HashMap::new();
    for row in fetch_all_rows_from_graph(query).await? {
        let user_id: String = row.get("user_id")?;
        let hs_id: String = row.get("hs_id")?;
        match PubkyId::try_from(&hs_id) {
            Ok(homeserver_id) => {
                homeservers.insert(user_id, homeserver_id);
            }
            Err(e) => warn!(
                "EventLineHomeserverBackfill migration: user {user_id} is hosted by {hs_id:?}, \
                 which is no public key ({e}); its lines are left without one"
            ),
        }
    }
    Ok(homeservers)
}

/// The rewrites of `keyless`: every line whose user is in `homeservers`, as `(index, line as
/// read, line with the key)`. The rest stay as they are.
fn rewrites(
    keyless: Vec<(u64, String)>,
    homeservers: &HashMap<String, PubkyId>,
) -> Vec<(u64, String, String)> {
    keyless
        .into_iter()
        .filter_map(|(index, line)| {
            let homeserver_id = homeservers.get(line_user_id(&line)?.as_ref())?;
            let rewritten = EventLine::new(&line, homeserver_id).as_str().to_string();
            Some((index, line, rewritten))
        })
        .collect()
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

            let keyless = keyless_lines(cursor, &lines);
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
            let rewrites = rewrites(keyless, &homeservers);
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

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_common::{StackConfig, StackManager};

    const USER: &str = "uo7jgkykft4885n8cruizwy6khw71mnu5pq3ay9i8pw1ymcn85ko";
    const OTHER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
    const HS: &str = "8rsrmfrn1anbrzuxiffwy1174o58emf4qgbfk5h7s8a33r3bd8dy";

    fn homeserver() -> PubkyId {
        PubkyId::try_from(HS).unwrap()
    }

    fn post_line(user: &str) -> String {
        format!("PUT pubky://{user}/pub/pubky.app/posts/0032SSN7Q4EVG")
    }

    fn keyed(line: &str) -> String {
        format!("{line} {HS}")
    }

    /// Lines are told apart as [`EventLine::split`] reads them, and keep their index in the
    /// list: a follow's key-shaped tail is no key, a keyed address with spaces is keyed.
    #[test]
    fn keyless_lines_keeps_the_lines_without_a_key_and_their_index() {
        let follow = format!("DEL pubky://{USER}/pub/pubky.app/follows/{OTHER}");
        let lines = vec![
            keyed(&post_line(USER)),
            post_line(USER),
            follow.clone(),
            keyed(&format!(
                "PUT pubky://{USER}/pub/pubky.app/files/my holiday photo.jpg"
            )),
            "garbage".to_string(),
        ];

        assert_eq!(
            keyless_lines(1000, &lines),
            vec![
                (1001, post_line(USER)),
                (1002, follow),
                (1004, "garbage".to_string())
            ]
        );
        assert!(keyless_lines(0, &[]).is_empty());
    }

    #[test]
    fn line_user_id_reads_the_user_of_a_keyed_or_keyless_line() {
        let user = PubkyId::try_from(USER).unwrap();
        assert_eq!(line_user_id(&post_line(USER)), Some(user.clone()));
        assert_eq!(line_user_id(&keyed(&post_line(USER))), Some(user.clone()));
        assert_eq!(
            line_user_id(&format!("DEL pubky://{USER}/pub/pubky.app/profile.json")),
            Some(user)
        );
    }

    #[test]
    fn line_user_id_is_none_for_a_line_without_a_user() {
        for line in [
            "garbage".to_string(),
            "PUT".to_string(),
            "PUT https://example.com/pub/x".to_string(),
            "PUT pubky://not-a-key/pub/x".to_string(),
            format!("PUT pubky://{USER}"),
        ] {
            assert_eq!(line_user_id(&line), None, "{line}");
        }
    }

    /// A line gets its user's homeserver; a line whose user has none, or which names no user,
    /// is left out, so the caller counts it as left without a key.
    #[test]
    fn rewrites_keys_the_lines_of_users_with_a_homeserver() {
        let homeservers = HashMap::from([(USER.to_string(), homeserver())]);
        let keyless = vec![
            (3, post_line(USER)),
            (4, post_line(OTHER)),
            (5, "garbage".to_string()),
            (6, format!("DEL pubky://{USER}/pub/pubky.app/profile.json")),
        ];

        assert_eq!(
            rewrites(keyless, &homeservers),
            vec![
                (3, post_line(USER), keyed(&post_line(USER))),
                (
                    6,
                    format!("DEL pubky://{USER}/pub/pubky.app/profile.json"),
                    keyed(&format!("DEL pubky://{USER}/pub/pubky.app/profile.json"))
                ),
            ]
        );
    }

    /// Against a scratch list: an entry is rewritten only while it still holds the line that
    /// was read, so a line changed since is left alone and the count says how many were set.
    #[tokio_shared_rt::test(shared)]
    async fn rewrite_lines_sets_only_the_entries_still_holding_what_was_read(
    ) -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let key = "MigrationTest:EventLineHomeserverBackfill:Events";
        let lines = [post_line(USER), post_line(OTHER), "garbage".to_string()];
        let mut conn = get_redis_conn().await?;
        redis::cmd("DEL")
            .arg(key)
            .query_async::<()>(&mut conn)
            .await?;
        redis::cmd("RPUSH")
            .arg(key)
            .arg(&lines)
            .query_async::<()>(&mut conn)
            .await?;

        let set = rewrite_lines(
            key,
            &[
                (0, lines[0].clone(), keyed(&lines[0])),
                (1, "not what the list holds".to_string(), keyed(&lines[1])),
                (2, lines[2].clone(), keyed(&lines[2])),
            ],
        )
        .await?;

        let stored: Vec<String> = redis::cmd("LRANGE")
            .arg(key)
            .arg(0)
            .arg(-1)
            .query_async(&mut conn)
            .await?;
        redis::cmd("DEL")
            .arg(key)
            .query_async::<()>(&mut conn)
            .await?;
        assert_eq!(set, 2);
        assert_eq!(
            stored,
            vec![keyed(&lines[0]), lines[1].clone(), keyed(&lines[2])]
        );
        Ok(())
    }
}
