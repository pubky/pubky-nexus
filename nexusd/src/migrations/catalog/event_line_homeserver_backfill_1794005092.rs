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
/// knows it, by swapping a keyed copy in for the list they are stored in and keeping the list
/// as it was beside it.
///
/// # What this does
/// Builds a copy of the `Events` list, line for line and at the same offsets, as
/// `Event:Events_new`. A line without a key, read as [`EventLine::split`] reads it, is copied
/// with the key of the homeserver its user's `HOSTED_BY` edge points to in front. A line whose
/// user has no edge, or whose user can't be read, is copied as it is, and so is a line a newer
/// watcher already stored with a key.
///
/// Once the copy holds as many lines as the list, one script renames `Event:Events` to
/// `Event:Events_old` and the copy to `Event:Events`. Nothing stored is rewritten: the list as
/// it was stays in `Event:Events_old`.
///
/// The edge names the homeserver the user was bound to, not the one an old event was read
/// from. A user who moves keeps the edge: the resolver marks it stale instead of switching it,
/// and the stale flag is ignored here. So a user who moved after being bound gets the
/// homeserver they were bound to, and only a user first bound after moving gets the new one.
///
/// # While it runs
/// It is meant to run with every instance stopped (see Deploy ordering). A watcher left
/// running loses no line all the same: a line it appends meanwhile is copied like any other,
/// and the swap happens only at the moment both lists have the same length. Every line keeps
/// its offset, so the `/v0/events` cursors stay valid across the swap.
///
/// Each page's users are looked up in one query and its lines appended to the copy by one
/// script. An append costs the same wherever the copy ends. Reading a page makes Redis seek to
/// it from the nearer end of the list, once per page, which is the only cost that grows with
/// the list: at the middle of 2,000,000 lines the seek added 0.09 ms to a read of 0.07 ms.
///
/// # Idempotency
/// A page is appended only while the copy still has the length the page was read at, and a run
/// starts where the copy ends, so a failed run is safe to repeat. Once `Event:Events_old`
/// exists a run does nothing. `is_multi_staged()` is `false`, so the manager marks this Done
/// after one successful run: a line left without a key keeps none after its user gains an edge.
///
/// # Rollback
/// `Event:Events_old` is never written again and is never deleted here: it holds every line
/// stored before the swap, as it was. To go back, stop the watcher, append to it the lines
/// `Event:Events` holds past its length, each without its key ([`EventLine::split`]), and
/// rename it to `Event:Events`. Delete it (`DEL Event:Events_old`) once the rebuilt list is
/// trusted: until then the lines are stored twice.
///
/// # Deploy ordering
/// Hard cutover: stop every instance of the previous build (API and watcher), run
/// `nexusd db migration run`, then start this build. The steps must not overlap. A previous
/// API serves lines as stored, so after the swap it would hand keyed lines to readers that
/// expect `PUT` or `DEL` first, and a line a previous watcher appends after the swap stays
/// without a key.
pub struct EventLineHomeserverBackfill1794005092;

/// Event lines copied per page.
const EVENT_PAGE_SIZE: usize = 500;

/// The lists a rebuild works on, by name under [`EventLine`]'s prefix.
struct Lists<'a> {
    /// The list the watcher appends to and the API reads.
    live: &'a str,
    /// The keyed copy under construction. Gone once swapped in.
    copy: &'a str,
    /// The live list as it was, kept after the swap.
    original: &'a str,
}

const EVENT_LISTS: Lists<'static> = Lists {
    live: EventLine::LIST,
    copy: "Events_new",
    original: "Events_old",
};

/// The Redis key of `list`.
async fn list_key(list: &str) -> String {
    format!("{}:{list}", EventLine::prefix().await)
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

/// A page of the list as it goes into the copy.
#[derive(Debug, PartialEq)]
struct Page {
    /// The page's lines, in order, each with a key in front where one was given.
    lines: Vec<String>,
    /// Lines given a homeserver key.
    keyed: usize,
    /// Keyless lines copied as they were: no user, or a user without a homeserver.
    left: usize,
}

/// `lines` as the copy holds them: a keyless line gets the homeserver `homeservers` has for
/// its user, every other line is copied as it is.
fn with_keys(lines: Vec<String>, homeservers: &HashMap<String, PubkyId>) -> Page {
    let mut page = Page {
        lines: Vec::with_capacity(lines.len()),
        keyed: 0,
        left: 0,
    };
    for line in lines {
        if EventLine::split(&line).1.is_some() {
            page.lines.push(line);
            continue;
        }
        let homeserver_id =
            line_user_id(&line).and_then(|user_id| homeservers.get(user_id.as_ref()));
        match homeserver_id {
            Some(homeserver_id) => {
                page.keyed += 1;
                page.lines
                    .push(EventLine::new(&line, homeserver_id).as_str().to_string());
            }
            None => {
                page.left += 1;
                page.lines.push(line);
            }
        }
    }
    page
}

/// Appends `lines` to `list` if it holds exactly `length` entries, and says whether it did.
async fn append_if_length(list: &str, length: usize, lines: &[String]) -> Result<bool, DynError> {
    let script = Script::new(
        r#"
        if redis.call('LLEN', KEYS[1]) ~= tonumber(ARGV[1]) then
            return 0
        end
        redis.call('RPUSH', KEYS[1], unpack(ARGV, 2))
        return 1
    "#,
    );

    let mut invocation = script.prepare_invoke();
    invocation.key(list_key(list).await).arg(length);
    for line in lines {
        invocation.arg(line);
    }

    let mut conn = get_redis_conn().await?;
    Ok(invocation.invoke_async(&mut conn).await?)
}

/// What [`swap_if_caught_up`] found and did.
#[derive(Debug, PartialEq)]
enum Swap {
    /// The copy held every line of the live list: it is the live list now, and the list as
    /// it was is the original.
    Swapped,
    /// The live list holds lines the copy lacks. Nothing was renamed.
    Behind,
    /// An original is already kept: a swap happened before. Nothing was renamed.
    AlreadySwapped,
    /// Neither the live list nor the copy holds a line.
    NothingStored,
    /// The copy holds more lines than the live list, so it is no copy of it. Nothing was
    /// renamed.
    CopyAhead,
}

/// Renames the live list to the original and the copy to the live list, in one step, if the
/// copy holds exactly the lines of the live list and no original is kept yet.
async fn swap_if_caught_up(lists: &Lists<'_>) -> Result<Swap, DynError> {
    let script = Script::new(
        r#"
        if redis.call('EXISTS', KEYS[3]) == 1 then
            return 'already_swapped'
        end
        local live = redis.call('LLEN', KEYS[1])
        local copy = redis.call('LLEN', KEYS[2])
        if copy > live then
            return 'copy_ahead'
        end
        if copy < live then
            return 'behind'
        end
        if live == 0 then
            return 'nothing_stored'
        end
        redis.call('RENAME', KEYS[1], KEYS[3])
        redis.call('RENAME', KEYS[2], KEYS[1])
        return 'swapped'
    "#,
    );

    let mut conn = get_redis_conn().await?;
    let outcome: String = script
        .key(list_key(lists.live).await)
        .key(list_key(lists.copy).await)
        .key(list_key(lists.original).await)
        .invoke_async(&mut conn)
        .await?;
    match outcome.as_str() {
        "swapped" => Ok(Swap::Swapped),
        "behind" => Ok(Swap::Behind),
        "already_swapped" => Ok(Swap::AlreadySwapped),
        "nothing_stored" => Ok(Swap::NothingStored),
        "copy_ahead" => Ok(Swap::CopyAhead),
        other => Err(format!(
            "EventLineHomeserverBackfill migration: unknown swap outcome {other}"
        )
        .into()),
    }
}

/// What a run copied.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
struct Tally {
    /// Lines copied.
    read: u64,
    /// Lines given a homeserver key.
    keyed: usize,
    /// Keyless lines copied as they were: no user, or a user without a homeserver.
    left: usize,
}

impl Tally {
    /// Counts a page that went into the copy, and logs the progress.
    fn add(&mut self, page: &Page) {
        self.read += page.lines.len() as u64;
        self.keyed += page.keyed;
        self.left += page.left;
        info!(
            "EventLineHomeserverBackfill migration: {} event lines copied, {} given a \
             homeserver, {} left without one",
            self.read, self.keyed, self.left
        );
    }
}

/// How a rebuild ended.
#[derive(Debug, PartialEq)]
enum Rebuild {
    /// The keyed copy is the live list, and the list as it was is kept as the original.
    Swapped(Tally),
    /// An original is already kept: an earlier run swapped. Nothing was done.
    AlreadySwapped,
    /// No event line is stored. Nothing was done.
    NothingStored,
}

impl Swap {
    /// How a rebuild that copied `tally` ends on this outcome, or `None` when it goes on:
    /// the live list got a line since the last page was read.
    fn ends_rebuild(self, tally: Tally, lists: &Lists<'_>) -> Option<Result<Rebuild, DynError>> {
        match self {
            Swap::Behind => None,
            Swap::Swapped => Some(Ok(Rebuild::Swapped(tally))),
            Swap::AlreadySwapped => Some(Ok(Rebuild::AlreadySwapped)),
            Swap::NothingStored => Some(Ok(Rebuild::NothingStored)),
            Swap::CopyAhead => Some(Err(format!(
                "EventLineHomeserverBackfill migration: the list {} holds more lines than {}, \
                 so it is no copy of it. Delete it and run the migration again.",
                lists.copy, lists.live
            )
            .into())),
        }
    }
}

/// How many lines `list` holds. A list that doesn't exist holds none.
async fn list_length(list: &str) -> Result<usize, DynError> {
    let mut conn = get_redis_conn().await?;
    Ok(redis::cmd("LLEN")
        .arg(list_key(list).await)
        .query_async(&mut conn)
        .await?)
}

/// The lines of the live list the copy lacks, a page of them at most, and the offset they
/// start at: the length of the copy.
async fn uncopied_page(lists: &Lists<'_>) -> Result<(usize, Vec<String>), DynError> {
    let copied = list_length(lists.copy).await?;
    let lines = EventLine::try_from_index_list(&[lists.live], Some(copied), Some(EVENT_PAGE_SIZE))
        .await?
        .unwrap_or_default();
    Ok((copied, lines))
}

/// Appends `lines`, the page of the live list at offset `copied`, to the copy, each keyless
/// line with the homeserver of its user's `HOSTED_BY` edge. `None` when the copy no longer
/// ends at `copied`: another run appended first.
async fn copy_page(
    copy: &str,
    copied: usize,
    lines: Vec<String>,
) -> Result<Option<Page>, DynError> {
    let user_ids: BTreeSet<String> = lines
        .iter()
        .filter(|line| EventLine::split(line).1.is_none())
        .filter_map(|line| line_user_id(line))
        .map(|user_id| user_id.to_string())
        .collect();
    let homeservers = hosting_homeservers(user_ids.into_iter().collect()).await?;

    let page = with_keys(lines, &homeservers);
    let appended = append_if_length(copy, copied, &page.lines).await?;
    Ok(appended.then_some(page))
}

/// One step of a rebuild: copies the next page of the live list, or, when the copy lacks no
/// line, tries the swap. `Some` is how the rebuild ended.
async fn step(lists: &Lists<'_>, tally: &mut Tally) -> Result<Option<Rebuild>, DynError> {
    let (copied, lines) = uncopied_page(lists).await?;
    // The copy has every line this read saw. The swap checks it again and renames in the same
    // step, so a line appended in between sends the rebuild round once more.
    if lines.is_empty() {
        return swap_if_caught_up(lists)
            .await?
            .ends_rebuild(*tally, lists)
            .transpose();
    }

    // No page means another run appended first: the next step reads from the copy's new end.
    if let Some(page) = copy_page(lists.copy, copied, lines).await? {
        tally.add(&page);
    }
    Ok(None)
}

/// Copies the live list into the copy, a page at a time from where the copy ends, giving each
/// line without a key the homeserver of its user's `HOSTED_BY` edge, then swaps the copy in.
async fn rebuild(lists: &Lists<'_>) -> Result<Rebuild, DynError> {
    // Redis keeps no empty list, so an original that holds a line is one that exists.
    if list_length(lists.original).await? > 0 {
        return Ok(Rebuild::AlreadySwapped);
    }

    let mut tally = Tally::default();
    loop {
        if let Some(end) = step(lists, &mut tally).await? {
            return Ok(end);
        }
    }
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
        match rebuild(&EVENT_LISTS).await? {
            Rebuild::Swapped(Tally { read, keyed, left }) => info!(
                "EventLineHomeserverBackfill migration: complete — {read} event lines copied, \
                 {keyed} given a homeserver, {left} left without one. The list as it was is \
                 kept in {}",
                list_key(EVENT_LISTS.original).await
            ),
            Rebuild::AlreadySwapped => info!(
                "EventLineHomeserverBackfill migration: {} exists, the list was rebuilt before",
                list_key(EVENT_LISTS.original).await
            ),
            Rebuild::NothingStored => {
                info!("EventLineHomeserverBackfill migration: no event line is stored")
            }
        }
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
    use nexus_common::db::exec_single_row;
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

    /// `line` as stored with the key of `homeserver_id`.
    fn keyed_by(line: &str, homeserver_id: &str) -> String {
        EventLine::new(line, &PubkyId::try_from(homeserver_id).unwrap())
            .as_str()
            .to_string()
    }

    /// `line` as stored with the key of [`HS`].
    fn keyed(line: &str) -> String {
        keyed_by(line, HS)
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

    /// A page is copied line for line, in order: a keyless line gets its user's homeserver, a
    /// line whose user has none or which names no user is copied as it is, and so is a line a
    /// newer watcher already stored with a key, whatever its user's homeserver is now.
    #[test]
    fn with_keys_copies_every_line_and_keys_those_of_hosted_users() {
        let homeservers = HashMap::from([(USER.to_string(), homeserver())]);
        let follow = format!("DEL pubky://{USER}/pub/pubky.app/follows/{OTHER}");
        let already_keyed = keyed_by(&post_line(USER), OTHER);
        let page = with_keys(
            vec![
                post_line(USER),
                post_line(OTHER),
                "garbage".to_string(),
                follow.clone(),
                already_keyed.clone(),
            ],
            &homeservers,
        );

        assert_eq!(
            page,
            Page {
                lines: vec![
                    keyed(&post_line(USER)),
                    post_line(OTHER),
                    "garbage".to_string(),
                    keyed(&follow),
                    already_keyed,
                ],
                keyed: 2,
                left: 2,
            }
        );
    }

    /// The id is what the manager records as Done, and a single stage is why a line left
    /// without a key keeps none.
    #[test]
    fn the_migration_keeps_its_id_and_its_single_stage() {
        let migration = EventLineHomeserverBackfill1794005092;
        assert_eq!(migration.id(), "EventLineHomeserverBackfill1794005092");
        assert!(!migration.is_multi_staged());
    }

    /// The graph and list fixture of one test, under ids no other test or the mock data uses:
    /// a user hosted by a homeserver, a user with no homeserver, and a user absent from the
    /// graph.
    struct Fixture {
        edge_user: String,
        no_edge_user: String,
        absent_user: String,
        homeserver: String,
    }

    impl Fixture {
        /// `test` is spelled in z-base-32 characters, so the ids are valid keys.
        fn new(test: &str) -> Self {
            let id = |role: &str| {
                let id = format!("{:y<52}", format!("backfi11{test}{role}"));
                PubkyId::try_from(&id).expect("a valid fixture id");
                id
            };
            Self {
                edge_user: id("edge"),
                no_edge_user: id("noedge"),
                absent_user: id("absent"),
                homeserver: id("hs"),
            }
        }

        async fn delete(&self) -> Result<(), DynError> {
            exec_single_row(
                Query::new(
                    "event_line_homeserver_backfill_test_cleanup",
                    "MATCH (n) WHERE (n:User OR n:Homeserver) AND n.id IN $ids DETACH DELETE n",
                )
                .param(
                    "ids",
                    vec![
                        self.edge_user.as_str(),
                        self.no_edge_user.as_str(),
                        self.homeserver.as_str(),
                    ],
                ),
            )
            .await?;
            Ok(())
        }

        async fn seed(&self) -> Result<(), DynError> {
            self.delete().await?;
            exec_single_row(
                Query::new(
                    "event_line_homeserver_backfill_test_seed",
                    "CREATE (:User {id: $edge_user, name: 'migration test', indexed_at: 1})
                        -[:HOSTED_BY]->(:Homeserver {id: $hs})
                     CREATE (:User {id: $no_edge_user, name: 'migration test', indexed_at: 1})",
                )
                .param("edge_user", self.edge_user.as_str())
                .param("no_edge_user", self.no_edge_user.as_str())
                .param("hs", self.homeserver.as_str()),
            )
            .await?;
            Ok(())
        }

        /// The lines the list starts with, and what the rebuilt list holds for them.
        fn lines(&self) -> (Vec<String>, Vec<String>) {
            let lines = vec![
                post_line(&self.edge_user),
                post_line(&self.no_edge_user),
                post_line(&self.absent_user),
                keyed(&post_line(&self.edge_user)),
            ];
            let rebuilt = vec![
                keyed_by(&lines[0], &self.homeserver),
                lines[1].clone(),
                lines[2].clone(),
                lines[3].clone(),
            ];
            (lines, rebuilt)
        }
    }

    /// Three scratch lists of one test, under [`EventLine`]'s prefix.
    struct Scratch {
        live: String,
        copy: String,
        original: String,
    }

    impl Scratch {
        fn new(test: &str) -> Self {
            let list =
                |role: &str| format!("MigrationTest:EventLineHomeserverBackfill:{test}:{role}");
            Self {
                live: list("Live"),
                copy: list("Copy"),
                original: list("Original"),
            }
        }

        fn lists(&self) -> Lists<'_> {
            Lists {
                live: &self.live,
                copy: &self.copy,
                original: &self.original,
            }
        }

        async fn delete(&self) -> Result<(), DynError> {
            let mut conn = get_redis_conn().await?;
            for list in [&self.live, &self.copy, &self.original] {
                redis::cmd("DEL")
                    .arg(list_key(list).await)
                    .query_async::<()>(&mut conn)
                    .await?;
            }
            Ok(())
        }
    }

    async fn push(list: &str, lines: &[String]) -> Result<(), DynError> {
        let mut conn = get_redis_conn().await?;
        redis::cmd("RPUSH")
            .arg(list_key(list).await)
            .arg(lines)
            .query_async::<()>(&mut conn)
            .await?;
        Ok(())
    }

    /// The lines of `list`, or `None` when the list doesn't exist.
    async fn stored(list: &str) -> Result<Option<Vec<String>>, DynError> {
        let mut conn = get_redis_conn().await?;
        let key = list_key(list).await;
        let exists: bool = redis::cmd("EXISTS")
            .arg(&key)
            .query_async(&mut conn)
            .await?;
        if !exists {
            return Ok(None);
        }
        Ok(Some(
            redis::cmd("LRANGE")
                .arg(&key)
                .arg(0)
                .arg(-1)
                .query_async(&mut conn)
                .await?,
        ))
    }

    /// The rebuild over scratch lists and the graph. The live list ends up holding every line
    /// at its offset, keyed where its user has a `HOSTED_BY` edge; the list as it was is kept,
    /// untouched, as the original; the copy is gone. A second run changes nothing.
    #[tokio_shared_rt::test(shared)]
    async fn rebuild_swaps_in_the_keyed_list_and_keeps_the_original() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let fixture = Fixture::new("swap");
        fixture.seed().await?;
        let scratch = Scratch::new("Swap");
        scratch.delete().await?;
        let (lines, rebuilt) = fixture.lines();
        push(&scratch.live, &lines).await?;

        let first = rebuild(&scratch.lists()).await;
        let after_first = (
            stored(&scratch.live).await?,
            stored(&scratch.original).await?,
            stored(&scratch.copy).await?,
        );
        let second = rebuild(&scratch.lists()).await;
        let after_second = (
            stored(&scratch.live).await?,
            stored(&scratch.original).await?,
            stored(&scratch.copy).await?,
        );
        scratch.delete().await?;
        fixture.delete().await?;

        assert_eq!(
            first?,
            Rebuild::Swapped(Tally {
                read: 4,
                keyed: 1,
                left: 2
            })
        );
        assert_eq!(after_first, (Some(rebuilt), Some(lines), None));
        assert_eq!(second?, Rebuild::AlreadySwapped);
        assert_eq!(after_second, after_first);
        Ok(())
    }

    /// A run that stopped mid-copy left a partial copy behind. The next run resumes after it
    /// and reaches the same lists.
    #[tokio_shared_rt::test(shared)]
    async fn rebuild_resumes_a_partial_copy() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let fixture = Fixture::new("resume");
        fixture.seed().await?;
        let scratch = Scratch::new("Resume");
        scratch.delete().await?;
        let (lines, rebuilt) = fixture.lines();
        push(&scratch.live, &lines).await?;
        push(&scratch.copy, &rebuilt[..2]).await?;

        let outcome = rebuild(&scratch.lists()).await;
        let after = (
            stored(&scratch.live).await?,
            stored(&scratch.original).await?,
            stored(&scratch.copy).await?,
        );
        scratch.delete().await?;
        fixture.delete().await?;

        assert_eq!(
            outcome?,
            Rebuild::Swapped(Tally {
                read: 2,
                keyed: 0,
                left: 1
            })
        );
        assert_eq!(after, (Some(rebuilt), Some(lines), None));
        Ok(())
    }

    /// Once an original is kept, however short, a run copies nothing: the live list is the
    /// rebuilt one already.
    #[tokio_shared_rt::test(shared)]
    async fn rebuild_does_nothing_once_an_original_is_kept() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let scratch = Scratch::new("Kept");
        scratch.delete().await?;
        let live = vec![keyed(&post_line(USER)), post_line(OTHER)];
        let original = vec![post_line(USER)];
        push(&scratch.live, &live).await?;
        push(&scratch.original, &original).await?;

        let outcome = rebuild(&scratch.lists()).await;
        let after = (
            stored(&scratch.live).await?,
            stored(&scratch.original).await?,
            stored(&scratch.copy).await?,
        );
        scratch.delete().await?;

        assert_eq!(outcome?, Rebuild::AlreadySwapped);
        assert_eq!(after, (Some(live), Some(original), None));
        Ok(())
    }

    /// With no event line stored there is nothing to rebuild, and no list is created.
    #[tokio_shared_rt::test(shared)]
    async fn rebuild_of_an_empty_list_creates_nothing() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let scratch = Scratch::new("Empty");
        scratch.delete().await?;

        let outcome = rebuild(&scratch.lists()).await;
        let after = (
            stored(&scratch.live).await?,
            stored(&scratch.original).await?,
            stored(&scratch.copy).await?,
        );

        assert_eq!(outcome?, Rebuild::NothingStored);
        assert_eq!(after, (None, None, None));
        Ok(())
    }

    /// The swap happens only when the copy holds as many lines as the live list, so a line the
    /// watcher appended since the last page is never left behind, and it never replaces an
    /// original that is already kept.
    #[tokio_shared_rt::test(shared)]
    async fn swap_waits_for_the_copy_and_never_replaces_a_kept_original() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let scratch = Scratch::new("SwapGuard");
        scratch.delete().await?;
        let lists = scratch.lists();
        let (lines, rebuilt) = Fixture::new("guard").lines();
        push(&scratch.live, &lines).await?;

        push(&scratch.copy, &rebuilt[..3]).await?;
        let behind = swap_if_caught_up(&lists).await;
        let after_behind = (
            stored(&scratch.live).await?,
            stored(&scratch.original).await?,
        );

        push(&scratch.copy, &rebuilt[3..]).await?;
        push(&scratch.copy, &[post_line(USER)]).await?;
        let ahead = swap_if_caught_up(&lists).await;

        scratch.delete().await?;
        push(&scratch.live, &lines).await?;
        push(&scratch.copy, &rebuilt).await?;
        push(&scratch.original, &[post_line(OTHER)]).await?;
        let kept = swap_if_caught_up(&lists).await;
        let after_kept = (
            stored(&scratch.live).await?,
            stored(&scratch.original).await?,
        );
        scratch.delete().await?;

        assert_eq!(behind?, Swap::Behind);
        assert_eq!(after_behind, (Some(lines.clone()), None));
        assert_eq!(ahead?, Swap::CopyAhead);
        assert_eq!(kept?, Swap::AlreadySwapped);
        assert_eq!(after_kept, (Some(lines), Some(vec![post_line(OTHER)])));
        Ok(())
    }

    /// A page is appended only while the copy still has the length the page was read at, so
    /// two runs at once, or a rerun, can't append a page twice.
    #[tokio_shared_rt::test(shared)]
    async fn append_if_length_appends_only_at_the_length_the_page_was_read_at(
    ) -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let scratch = Scratch::new("Append");
        scratch.delete().await?;
        let (lines, _) = Fixture::new("append").lines();

        let first = append_if_length(&scratch.copy, 0, &lines[..2]).await;
        let stale = append_if_length(&scratch.copy, 0, &lines[..2]).await;
        let next = append_if_length(&scratch.copy, 2, &lines[2..]).await;
        let after = stored(&scratch.copy).await?;
        scratch.delete().await?;

        assert!(first?);
        assert!(!stale?);
        assert!(next?);
        assert_eq!(after, Some(lines));
        Ok(())
    }
}
