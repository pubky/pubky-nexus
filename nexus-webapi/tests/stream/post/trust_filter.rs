//! Posts by authors outside the trust ranking are kept out of the sorted sets
//! every viewer shares, and the Cypher-served `source=all` shapes apply the same
//! rule. An author's own ALL timeline (`viewer_id`, timeline, no tags) gets their
//! posts back.
//!
//! Fixture: trust.cypher ranks every user except the wot on-ramp accounts, so
//! inside the wot window (`indexed_at` 1650000000001..=1650000000014, used by no
//! other fixture) only D1, D1B and D2 are ranked. Newest first, the window's root
//! posts are DELETED_USER (014), ARTIST1 (012), BTC5..BTC1 (011..007), SPAMMER
//! (006), D2 (004), D1B (003), D1 (002) and OBSERVER (001).
use std::collections::HashSet;

use crate::utils::get_request;
use crate::utils::recommended::{D4, DELETED};
use crate::utils::search_reach::UNKNOWN_USER;
use crate::utils::server::TestServiceServer;
use anyhow::Result;
use deadpool_redis::redis::AsyncCommands;
use nexus_common::db::graph::Query;
use nexus_common::db::{fetch_key_from_graph, get_redis_conn};
use nexus_common::models::user::USER_SOCIAL_GRAPH_KEY_PARTS;
use serde_json::Value;

use super::utils::ids_in;
use super::{KEYS_ROOT_PATH, ROOT_PATH};

const WOT_D1: &str = "qjftuwjog819ki1wktuy5tndebce36bmxxwtjjm3z1fr97jk9yuo";
const OBSERVER: &str = "y6apowjmcg8rocmd9jirg95fyf3yykwuhqxozzts4mjipk4n7iao";
const WOT_D1B: &str = "t5ixbtatg4tq5q5ixg16qqrg1bmem75ksg6cweuftuydwzw91pzy";
const WOT_D2: &str = "smf4xrqfhx7stnufkjzhbjyu3rbgb3gga64srqmzcyyoyzefse9y";
const SPAMMER: &str = "qdsygndnk45m9ru5jseg3uxk5xg4usj9hrcraqbzgigapzweaa9o";

const D1_POST: &str = "WOTPOSTD10002";
const OBSERVER_POST: &str = "WOTPOSTO00001";
const D1B_POST: &str = "WOTPOSTD1B003";
const D2_POST: &str = "WOTPOSTD20004";
const SPAMMER_POST: &str = "WOTPOSTS00006";

const WINDOW_START: i64 = 1650000000014;
const WINDOW: &str = "source=all&sorting=timeline&end=1650000000001";

/// Viewers other than the hidden authors: none, ranked, unknown.
fn other_viewers() -> [String; 3] {
    [
        String::new(),
        format!("&viewer_id={WOT_D1}"),
        format!("&viewer_id={UNKNOWN_USER}"),
    ]
}

/// The ranked root posts in the window, newest first.
fn ranked_window_keys() -> Vec<String> {
    vec![
        format!("{WOT_D2}:{D2_POST}"),
        format!("{WOT_D1B}:{D1B_POST}"),
        format!("{WOT_D1}:{D1_POST}"),
    ]
}

async fn keys(query: &str) -> Result<Value> {
    Ok(get_request(&format!("{KEYS_ROOT_PATH}?{query}")).await?)
}

async fn posts(query: &str) -> Result<Value> {
    Ok(get_request(&format!("{ROOT_PATH}?{query}")).await?)
}

fn post_keys_in(response: &Value) -> Vec<String> {
    response["post_keys"]
        .as_array()
        .expect("post_keys array")
        .iter()
        .map(|key| key.as_str().unwrap_or_default().to_string())
        .collect()
}

/// The users in the trust ranking.
async fn ranking() -> Result<HashSet<String>> {
    TestServiceServer::get_test_server().await;
    let key = format!("Sorted:{}", USER_SOCIAL_GRAPH_KEY_PARTS.join(":"));
    let mut conn = get_redis_conn().await?;
    Ok(conn.zrange(key, 0, -1).await?)
}

/// The keys whose author is outside the ranking.
async fn unranked_keys(keys: &[String]) -> Result<Vec<String>> {
    let ranking = ranking().await?;
    Ok(keys
        .iter()
        .filter(|key| {
            let author = key.split_once(':').map(|(author, _)| author);
            !author.is_some_and(|author| ranking.contains(author))
        })
        .cloned()
        .collect())
}

/// Every key of `query`, read a page of 50 at a time with `skip`.
async fn walk(query: &str) -> Result<Vec<String>> {
    let mut all = Vec::new();
    loop {
        let page = post_keys_in(&keys(&format!("{query}&limit=50&skip={}", all.len())).await?);
        let done = page.len() < 50;
        all.extend(page);
        if done {
            return Ok(all);
        }
    }
}

/// Every key of `query`, read a page of one at a time with the score cursor
/// (`start = last_post_score - 1`) until an empty page, as pubky-app pages.
async fn walk_cursor(query: &str) -> Result<Vec<String>> {
    let mut start = WINDOW_START;
    let mut all = Vec::new();
    loop {
        let page = keys(&format!("{query}&start={start}&limit=1")).await?;
        let page_keys = post_keys_in(&page);
        if page_keys.is_empty() {
            return Ok(all);
        }
        all.extend(page_keys);
        start = page["last_post_score"].as_i64().expect("cursor") - 1;
    }
}

/// Every shape serves other viewers only the ranked authors, on the keys route
/// and the hydrated one. The plain stream reads the global timeline; `kind` and
/// `exclude_kinds` go to Cypher, which keeps the authors the ranking would.
#[tokio_shared_rt::test(shared)]
async fn test_all_timeline_hides_unranked_authors_from_others() -> Result<()> {
    for viewer in other_viewers() {
        for shape in ["", "&kind=short", "&exclude_kinds=long"] {
            let query = format!("{WINDOW}{shape}&start={WINDOW_START}&limit=50{viewer}");
            let page = keys(&query).await?;
            assert_eq!(post_keys_in(&page), ranked_window_keys(), "{shape}{viewer}");
            let cursor = Value::from(1650000000002_u64);
            assert_eq!(page["last_post_score"], cursor, "{shape}{viewer}");

            let page = posts(&query).await?;
            let expected = [D2_POST, D1B_POST, D1_POST];
            assert_eq!(ids_in(&page), expected, "{shape}{viewer}");
        }
    }

    // Several tags go to Cypher too: the rule composes with the tag MATCH
    // rather than emptying the stream.
    let tagged = walk("source=all&sorting=timeline&tags=bitcoin,opensource").await?;
    assert!(!tagged.is_empty());
    assert!(unranked_keys(&tagged).await?.is_empty(), "{tagged:?}");
    Ok(())
}

/// An unranked author's own ALL timeline gets their root posts back, on every
/// shape, wherever they fall in the window: the spammer's post tops it, the
/// observer's ends it.
#[tokio_shared_rt::test(shared)]
async fn test_unranked_viewer_gets_own_posts_back() -> Result<()> {
    let spammer_post = format!("{SPAMMER}:{SPAMMER_POST}");
    let observer_post = format!("{OBSERVER}:{OBSERVER_POST}");
    let ranked = ranked_window_keys();
    let spammer_window = [vec![spammer_post.clone()], ranked.clone()].concat();
    let observer_window = [ranked, vec![observer_post]].concat();
    for (viewer, expected) in [(SPAMMER, &spammer_window), (OBSERVER, &observer_window)] {
        for shape in ["", "&kind=short", "&exclude_kinds=long"] {
            let query = format!("{WINDOW}{shape}&start={WINDOW_START}&limit=50&viewer_id={viewer}");
            assert_eq!(&post_keys_in(&keys(&query).await?), expected, "{shape}");
        }
        let query = format!("{WINDOW}&start={WINDOW_START}&limit=50&viewer_id={viewer}");
        let ids: Vec<String> = expected
            .iter()
            .map(|key| key.split_once(':').expect("author:post").1.to_string())
            .collect();
        assert_eq!(ids_in(&posts(&query).await?), ids);
    }

    // Pages stay exact around the merged post: a walk, `skip` and ascending order.
    let viewer = format!("viewer_id={SPAMMER}");
    assert_eq!(
        walk_cursor(&format!("{WINDOW}&{viewer}")).await?,
        spammer_window
    );

    let page = keys(&format!(
        "{WINDOW}&start={WINDOW_START}&skip=1&limit=2&{viewer}"
    ))
    .await?;
    assert_eq!(post_keys_in(&page), spammer_window[1..3].to_vec());

    let page = keys(&format!(
        "{WINDOW}&start={WINDOW_START}&order=ascending&limit=50&{viewer}"
    ))
    .await?;
    let mut ascending = spammer_window;
    ascending.reverse();
    assert_eq!(post_keys_in(&page), ascending);

    // Only the timeline: the author's tag and engagement streams stay filtered.
    let page = keys(&format!("source=all&sorting=timeline&tags=wmtag1&{viewer}")).await?;
    assert!(post_keys_in(&page).is_empty(), "{page}");
    let hot = walk(&format!("source=all&sorting=total_engagement&{viewer}")).await?;
    assert!(!hot.contains(&spammer_post));
    Ok(())
}

const DELETED_START: i64 = 1600000001034;
const DELETED_END: i64 = 1600000001029;

/// A deleted profile keeps its node, its posts and the score a recompute gives
/// it. The ranking skips deleted users and the Cypher rule runs the same test,
/// so its posts are hidden on every shape. recommended.cypher's deleted user has
/// five root posts (`indexed_at` 1600000001030..=034) above a ranked one (029).
#[tokio_shared_rt::test(shared)]
async fn test_all_timeline_hides_deleted_authors() -> Result<()> {
    // Unscored, the trust check alone would hide it, and this test would pass
    // without the deletion check.
    TestServiceServer::get_test_server().await;
    let query = Query::new(
        "test_user_trust",
        "MATCH (u:User {id: $id}) RETURN u.trust AS trust",
    )
    .param("id", DELETED);
    let trust = fetch_key_from_graph::<Option<f64>>(query, "trust")
        .await?
        .flatten();
    assert!(trust.is_some_and(|trust| trust > 0.0), "{trust:?}");

    // Its posts are there, on its own stream.
    let own = keys(&format!(
        "source=author&author_id={DELETED}&start={DELETED_START}&end={DELETED_END}&limit=50"
    ))
    .await?;
    let deleted_posts: Vec<String> = (1..=5)
        .rev()
        .map(|n| format!("{DELETED}:RECPOSTDEL00{n}"))
        .collect();
    assert_eq!(post_keys_in(&own), deleted_posts);

    let ranked = format!("{D4}:RECPOSTD4X005");
    for shape in ["", "&kind=short", "&exclude_kinds=long"] {
        let query = format!(
            "source=all&sorting=timeline&start={DELETED_START}&end={DELETED_END}&limit=50{shape}"
        );
        assert_eq!(
            post_keys_in(&keys(&query).await?),
            [ranked.as_str()],
            "{shape}"
        );
    }
    Ok(())
}

/// pubky-app pages with `start = last_post_score - 1` and treats a short page
/// as the end of the feed, so the Cypher rule must filter before the limit: a
/// page of one is full until the ranked posts run out.
#[tokio_shared_rt::test(shared)]
async fn test_cypher_pages_stay_full() -> Result<()> {
    let served = walk_cursor(&format!("{WINDOW}&kind=short")).await?;
    assert_eq!(served, ranked_window_keys());
    Ok(())
}

/// The hot feed hides unranked authors too, served from the global engagement
/// set and from Cypher (`kind=short`).
#[tokio_shared_rt::test(shared)]
async fn test_engagement_feed_hides_unranked_authors() -> Result<()> {
    let d1_post = format!("{WOT_D1}:{D1_POST}");
    for shape in ["", "&kind=short"] {
        let served = walk(&format!("source=all&sorting=total_engagement{shape}")).await?;
        assert!(served.contains(&d1_post), "{shape}");
        let unranked = unranked_keys(&served).await?;
        assert!(unranked.is_empty(), "{shape}: {unranked:?}");
    }
    Ok(())
}

/// A tag's streams hide unranked authors on both sortings: D2's tagged reply
/// stays (tag streams carry replies), the spammer's tagged reply goes.
#[tokio_shared_rt::test(shared)]
async fn test_tag_streams_hide_unranked_authors() -> Result<()> {
    let page = keys("source=all&sorting=timeline&tags=nudity").await?;
    assert_eq!(post_keys_in(&page), [format!("{WOT_D2}:WOTPOSTREPLY1")]);

    for sorting in ["timeline", "total_engagement"] {
        let query = format!("source=all&sorting={sorting}&tags=wmtag1");
        let page = keys(&query).await?;
        assert!(post_keys_in(&page).is_empty(), "{sorting}: {page}");
        let page = posts(&query).await?;
        assert!(ids_in(&page).is_empty(), "{sorting}: {page}");
    }
    Ok(())
}

/// A thread hides the replies of unranked authors: the spammer's reply under
/// D2's post goes, D2's own stays.
#[tokio_shared_rt::test(shared)]
async fn test_threads_hide_unranked_replies() -> Result<()> {
    let thread = format!("source=post_replies&author_id={WOT_D2}&post_id=WOTPOSTTAGS01");
    let page = keys(&thread).await?;
    assert_eq!(post_keys_in(&page), [format!("{WOT_D2}:WOTPOSTCYCLE1")]);
    let page = posts(&thread).await?;
    assert_eq!(ids_in(&page), ["WOTPOSTCYCLE1"]);
    Ok(())
}
