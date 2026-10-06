use crate::migrations::manager::Migration;
use async_trait::async_trait;
use futures::StreamExt;
use nexus_common::db::get_neo4j_graph;
use nexus_common::db::graph::Query;
use nexus_common::models::event::EventLine;
use nexus_common::types::DynError;
use nexus_watcher::events::event::{Event, EventType, ParseResult};
use pubky_app_specs::Resource;
use std::collections::BTreeMap;
use tracing::info;

/// Fill the `uri` property on posts, users and tag edges indexed before the watcher stored it.
///
/// # What this does
/// 1. `Post.uri` = `pubky://<author>/pub/pubky.app/posts/<id>`, for every post with an author.
/// 2. From the `Event:Events` LIST:
///    - `uri` of a `TAGGED` edge to a `Post` or `User` = the path of its tag file, read from the
///      file's PUT line. These edges have no `app`, so the folder the file sits in
///      (`pubky.app`, `mapky`, …) is only known from its event line.
///    - `User.uri` = the path of the user's profile.json, read from its PUT line.
/// 3. Every tag edge still without a `uri` = `pubky://<tagger>/pub/<folder>/tags/<t.id>`: the
///    edge's `app` for a `Resource` edge, `pubky.app` for a post or user edge with no event line
///    (older than the log, or lost in a flush), the address readers built for it before.
///
/// Afterwards every post with an author and every tag edge has a `uri`, and readers read it
/// directly: no address is built at read time. A user with no profile.json line (a stub never
/// read from a profile, or a profile older than the log) keeps no `uri`; readers fall back to
/// its `pubky.app` address.
///
/// # Step 2
/// One tag file, one edge: the edge's address is its file's path, as the watcher stores it on
/// every tag PUT. The list holds one line per successfully handled event and is never trimmed.
/// It is paged with `LRANGE` and read only; each tag PUT line gives its edge `(tagger, tag id)`
/// its address (see [`fold_page`]). A DEL needs no handling: an edge that exists now was
/// recreated by a PUT of the same file, which has the same address.
///
/// Each profile.json PUT line gives its user the same address. A profile DEL needs no handling
/// either: it deletes the node, or leaves a tombstone, which keeps the stored `uri`.
///
/// # Idempotency
/// Steps 1 and 3 only touch rows whose `uri` is null. Step 2 writes the same values from the
/// same log. A failed run is safe to repeat; `is_multi_staged()` is `false`, so the manager
/// marks this Done after one successful backfill.
///
/// # Deploy ordering
/// Stop every instance, run `nexusd db migration run`, then start the new binaries. The new
/// readers need every `uri` in place: a post without one fails to load. Users are the
/// exception: a user without one reads as its `pubky.app` address.
pub struct UriBackfill1791196997;

/// Event lines read per `LRANGE` page.
const EVENT_PAGE_SIZE: usize = 1_000;

/// A tag edge as the event log names it: `(tagger id, tag id)`.
type EdgeKey = (String, String);

/// What one page of the event log addresses.
#[derive(Debug, Default, PartialEq)]
struct PageUris {
    /// Tag edge → its file's path.
    edges: BTreeMap<EdgeKey, String>,
    /// User id → its profile.json path.
    profiles: BTreeMap<String, String>,
}

/// Maps each tag PUT line in a page to its edge and its file's path, and each profile.json PUT
/// line to its user and the file's path. Other lines, and lines that don't parse, are skipped.
fn fold_page(lines: &[String]) -> PageUris {
    let mut page = PageUris::default();
    for line in lines {
        let Ok(ParseResult::Parsed(event)) = Event::parse_event(line) else {
            continue;
        };
        if event.event_type != EventType::Put {
            continue;
        }
        let user_id = event.parsed_uri.user_id().to_string();
        match event.parsed_uri.resource() {
            Resource::Tag(tag_id) => {
                page.edges.insert((user_id, tag_id.clone()), event.uri);
            }
            Resource::User => {
                page.profiles.insert(user_id, event.uri);
            }
            _ => {}
        }
    }
    page
}

/// Runs a query that returns one `count` column and reads it.
async fn run_count(query: Query, column: &str) -> Result<i64, DynError> {
    let graph = get_neo4j_graph()?;
    let mut result = graph.execute(query).await?;
    match result.next().await {
        Some(Ok(row)) => Ok(row.get::<i64>(column)?),
        Some(Err(e)) => Err(e.into()),
        None => Err(format!("UriBackfill migration: query returned no rows ({column})").into()),
    }
}

/// Sets the address of the post and user tag edges a page names.
fn apply_page_query(edges: BTreeMap<EdgeKey, String>) -> Query {
    let rows: Vec<Vec<String>> = edges
        .into_iter()
        .map(|((tagger_id, tag_id), uri)| vec![tagger_id, tag_id, uri])
        .collect();

    // A row whose edge no longer exists matches nothing.
    Query::new(
        "uri_backfill_tag_edges",
        "UNWIND $rows AS row
         MATCH (:User {id: row[0]})-[t:TAGGED {id: row[1]}]->(target)
         WHERE target:Post OR target:User
         SET t.uri = row[2]
         RETURN count(t) AS matched",
    )
    .param("rows", rows)
}

/// Sets the address of the users whose profile.json a page names.
fn apply_profiles_query(profiles: BTreeMap<String, String>) -> Query {
    let rows: Vec<Vec<String>> = profiles
        .into_iter()
        .map(|(user_id, uri)| vec![user_id, uri])
        .collect();

    // A row whose user no longer exists matches nothing.
    Query::new(
        "uri_backfill_users",
        "UNWIND $rows AS row
         MATCH (u:User {id: row[0]})
         SET u.uri = row[1]
         RETURN count(u) AS matched",
    )
    .param("rows", rows)
}

#[async_trait]
impl Migration for UriBackfill1791196997 {
    fn id(&self) -> &'static str {
        "UriBackfill1791196997"
    }

    fn is_multi_staged(&self) -> bool {
        false
    }

    async fn dual_write(_data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        // Step 1: posts. MATCH stays outside the subquery so IN TRANSACTIONS batches its rows.
        let posts = run_count(
            Query::new(
                "uri_backfill_posts",
                "MATCH (u:User)-[:AUTHORED]->(p:Post)
                 WHERE p.uri IS NULL
                 CALL (u, p) {
                     SET p.uri = 'pubky://' + u.id + '/pub/pubky.app/posts/' + p.id
                 } IN TRANSACTIONS OF 10000 ROWS
                 RETURN count(p) AS processed",
            ),
            "processed",
        )
        .await?;
        info!("UriBackfill migration: {posts} posts given a uri");

        // Step 2: edges to a Post or User, and users, from the event log
        let mut cursor: u64 = 0;
        let mut lines_read: u64 = 0;
        let mut edges_matched: u64 = 0;
        let mut users_matched: u64 = 0;
        loop {
            let (lines, next_cursor) =
                EventLine::get_from_index(Some(cursor), EVENT_PAGE_SIZE).await?;
            if lines.is_empty() {
                break;
            }
            lines_read += lines.len() as u64;
            cursor = next_cursor;

            let page = fold_page(&lines);
            if !page.edges.is_empty() {
                edges_matched += run_count(apply_page_query(page.edges), "matched").await? as u64;
            }
            if !page.profiles.is_empty() {
                users_matched +=
                    run_count(apply_profiles_query(page.profiles), "matched").await? as u64;
            }
            info!(
                "UriBackfill migration: {lines_read} event lines read, {edges_matched} post and user tag edges and {users_matched} users matched"
            );

            if lines.len() < EVENT_PAGE_SIZE {
                break;
            }
        }

        // Step 3: every tag edge still without a uri, from the folder recorded on the edge.
        // Post and user edges record none, so they get the pubky.app folder.
        let folder_edges = run_count(
            Query::new(
                "uri_backfill_folder_tag_edges",
                "MATCH (tagger:User)-[t:TAGGED]->()
                 WHERE t.uri IS NULL AND t.id IS NOT NULL
                 CALL (tagger, t) {
                     SET t.uri = 'pubky://' + tagger.id + '/pub/' + coalesce(t.app, 'pubky.app')
                         + '/tags/' + t.id
                 } IN TRANSACTIONS OF 10000 ROWS
                 RETURN count(t) AS processed",
            ),
            "processed",
        )
        .await?;
        info!("UriBackfill migration: {folder_edges} tag edges given the address of their folder");

        // Nothing may be left behind: the readers no longer build an address. Erroring keeps
        // the migration not Done. A post with no author, or an edge with no id, has no address.
        // Users are not checked: one without a profile.json line keeps no `uri` by design.
        let remaining = run_count(
            Query::new(
                "uri_backfill_verify",
                "CALL () {
                     MATCH (:User)-[:AUTHORED]->(p:Post) WHERE p.uri IS NULL RETURN count(p) AS n
                     UNION ALL
                     MATCH ()-[t:TAGGED]->() WHERE t.uri IS NULL AND t.id IS NOT NULL
                     RETURN count(t) AS n
                 }
                 RETURN sum(n) AS remaining",
            ),
            "remaining",
        )
        .await?;
        if remaining != 0 {
            return Err(format!(
                "UriBackfill migration: {remaining} posts or tag edges still have no uri. \
                 Confirm every instance is stopped, then re-run the migration."
            )
            .into());
        }

        info!(
            "UriBackfill migration: complete — {posts} posts, {edges_matched} post and user tag edges and {users_matched} users matched from {lines_read} event lines, {folder_edges} tag edges from their folder"
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
    use pubky_app_specs::traits::HashId;
    use pubky_app_specs::PubkyAppTag;

    const TAGGER: &str = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";
    const AUTHOR: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";

    fn tag_id(label: &str) -> String {
        PubkyAppTag {
            uri: format!("pubky://{AUTHOR}/pub/pubky.app/posts/0032SSN7Q4EVG"),
            label: label.to_string(),
            created_at: 0,
        }
        .create_id()
    }

    fn tag_uri(app: &str, id: &str) -> String {
        format!("pubky://{TAGGER}/pub/{app}/tags/{id}")
    }

    fn key(id: &str) -> EdgeKey {
        (TAGGER.to_string(), id.to_string())
    }

    #[test]
    fn put_gives_its_edge_the_file_path() {
        let id = tag_id("one");
        let edges = fold_page(&[format!("PUT {}", tag_uri("pubky.app", &id))]).edges;
        assert_eq!(
            edges,
            BTreeMap::from([(key(&id), tag_uri("pubky.app", &id))])
        );
    }

    #[test]
    fn put_in_another_app_folder_keeps_that_folder() {
        let id = tag_id("two");
        let edges = fold_page(&[format!("PUT {}", tag_uri("mapky", &id))]).edges;
        assert_eq!(edges, BTreeMap::from([(key(&id), tag_uri("mapky", &id))]));
    }

    #[test]
    fn del_lines_are_ignored() {
        let id = tag_id("three");
        let edges = fold_page(&[
            format!("PUT {}", tag_uri("mapky", &id)),
            format!("DEL {}", tag_uri("mapky", &id)),
            format!("DEL {}", tag_uri("pubky.app", &tag_id("four"))),
        ])
        .edges;
        assert_eq!(edges, BTreeMap::from([(key(&id), tag_uri("mapky", &id))]));
    }

    #[test]
    fn other_and_malformed_lines_are_skipped() {
        let page = fold_page(&[
            format!("PUT pubky://{TAGGER}/pub/pubky.app/posts/0032SSN7Q4EVG"),
            format!("PUT pubky://{TAGGER}/pub/pubky.app/follows/{AUTHOR}"),
            "garbage".to_string(),
            "PUT".to_string(),
            "MOVE pubky://x/pub/pubky.app/tags/y".to_string(),
        ]);
        assert_eq!(page, PageUris::default());
    }

    #[test]
    fn profile_put_gives_its_user_the_file_path() {
        let profile = format!("pubky://{TAGGER}/pub/pubky.app/profile.json");
        let page = fold_page(&[
            format!("PUT {profile}"),
            format!("DEL pubky://{AUTHOR}/pub/pubky.app/profile.json"),
        ]);
        assert!(page.edges.is_empty(), "expected no edges, got {page:?}");
        assert_eq!(
            page.profiles,
            BTreeMap::from([(TAGGER.to_string(), profile)])
        );
    }

    #[test]
    fn edges_are_kept_apart_by_tag_id() {
        let first = tag_id("five");
        let second = tag_id("six");
        let edges = fold_page(&[
            format!("PUT {}", tag_uri("pubky.app", &first)),
            format!("PUT {}", tag_uri("mapky", &second)),
        ])
        .edges;
        assert_eq!(
            edges,
            BTreeMap::from([
                (key(&first), tag_uri("pubky.app", &first)),
                (key(&second), tag_uri("mapky", &second)),
            ])
        );
    }
}
