//! `source=all` with `sorting=timeline` hides posts by authors outside the
//! trust ranking, for requests without a viewer and for ranked viewers. A
//! viewer outside the ranking, or one Nexus does not know, gets the unfiltered
//! stream on every shape, Cypher included.
//!
//! Fixture: trust.cypher ranks every user except the wot on-ramp accounts, so
//! inside the wot window (`indexed_at` 1650000000001..=1650000000014, used by no
//! other fixture) only D1, D1B and D2 are ranked. Newest first, the window's root
//! posts are DELETED_USER (014), ARTIST1 (012), BTC5..BTC1 (011..007), SPAMMER
//! (006), D2 (004), D1B (003), D1 (002) and OBSERVER (001): a run of eight hidden
//! posts above the ranked ones and one below.
//!
//! These tests depend on the ranking key existing, which `test_social_graph_status`
//! briefly deletes; `.config/nextest.toml` runs that test alone.
use crate::utils::get_request;
use crate::utils::recommended::{D4, DELETED};
use crate::utils::search_reach::UNKNOWN_USER;
use crate::utils::server::TestServiceServer;
use anyhow::Result;
use nexus_common::db::fetch_key_from_graph;
use nexus_common::db::graph::Query;
use nexus_common::db::kv::SortOrder;
use nexus_common::models::post::{KindFilter, PostStream, StreamSource};
use nexus_common::types::{Pagination, StreamSorting};
use pubky_app_specs::PubkyAppPostKind;
use serde_json::Value;

use super::utils::ids_in;
use super::{KEYS_ROOT_PATH, ROOT_PATH};

const WOT_D1: &str = "qjftuwjog819ki1wktuy5tndebce36bmxxwtjjm3z1fr97jk9yuo";
const WOT_D1B: &str = "t5ixbtatg4tq5q5ixg16qqrg1bmem75ksg6cweuftuydwzw91pzy";
const WOT_D2: &str = "smf4xrqfhx7stnufkjzhbjyu3rbgb3gga64srqmzcyyoyzefse9y";
const SPAMMER: &str = "qdsygndnk45m9ru5jseg3uxk5xg4usj9hrcraqbzgigapzweaa9o";

const D1_POST: &str = "WOTPOSTD10002";
const D1B_POST: &str = "WOTPOSTD1B003";
const D2_POST: &str = "WOTPOSTD20004";
const SPAMMER_POST: &str = "WOTPOSTS00006";

const WINDOW_START: i64 = 1650000000014;
const WINDOW_END: i64 = 1650000000001;
const WINDOW: &str = "source=all&sorting=timeline&end=1650000000001";

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

/// A first page of `limit`, between the `start` and `end` scores when given.
fn page_between(start: Option<i64>, end: Option<i64>, limit: usize) -> Pagination {
    Pagination {
        start: start.map(|start| start as f64),
        end: end.map(|end| end as f64),
        skip: Some(0),
        limit: Some(limit),
    }
}

/// The stream exactly as served without the trust filter.
async fn unfiltered_keys(
    tags: Option<&[&str]>,
    kind: Option<KindFilter>,
    pagination: Pagination,
) -> Result<Vec<String>> {
    // The model call needs the stack the test server sets up.
    TestServiceServer::get_test_server().await;
    let stream = PostStream::get_post_keys(
        StreamSource::All,
        pagination,
        SortOrder::Descending,
        StreamSorting::Timeline,
        tags.map(|tags| tags.iter().map(ToString::to_string).collect()),
        kind,
        None,
    )
    .await?;
    Ok(stream.map(|stream| stream.post_keys).unwrap_or_default())
}

/// No viewer: every shape serves only the ranked authors, on the keys route and
/// the hydrated one. The plain stream reads the ranked set; `kind`,
/// `exclude_kinds` and several tags go to Cypher, which keeps the authors the
/// ranking would: a positive trust score and a profile that isn't deleted.
#[tokio_shared_rt::test(shared)]
async fn test_all_timeline_hides_unranked_authors() -> Result<()> {
    for shape in ["", "&kind=short", "&exclude_kinds=long"] {
        let page = keys(&format!("{WINDOW}{shape}&start={WINDOW_START}&limit=50")).await?;
        assert_eq!(post_keys_in(&page), ranked_window_keys(), "{shape}");
        let cursor = Value::from(1650000000002_u64);
        assert_eq!(page["last_post_score"], cursor, "{shape}");

        let page = posts(&format!("{WINDOW}{shape}&start={WINDOW_START}&limit=50")).await?;
        assert_eq!(ids_in(&page), [D2_POST, D1B_POST, D1_POST], "{shape}");
    }

    // Every fixture author tagging these is ranked; this pins that the rule
    // composes with the tag MATCH rather than emptying the stream.
    let tags = "tags=bitcoin,opensource&limit=50";
    let page = keys(&format!("source=all&sorting=timeline&{tags}")).await?;
    let unfiltered = unfiltered_keys(
        Some(&["bitcoin", "opensource"]),
        None,
        page_between(None, None, 50),
    )
    .await?;
    assert!(!unfiltered.is_empty());
    assert_eq!(post_keys_in(&page), unfiltered);
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

    let ranked = format!("{D4}:RECPOSTD4X005");
    let mut window_keys: Vec<String> = (1..=5)
        .rev()
        .map(|n| format!("{DELETED}:RECPOSTDEL00{n}"))
        .collect();
    window_keys.push(ranked.clone());
    let window = page_between(Some(DELETED_START), Some(DELETED_END), 50);
    for (shape, kind) in [
        ("", None),
        (
            "&kind=short",
            Some(KindFilter::Kind(PubkyAppPostKind::Short)),
        ),
        (
            "&exclude_kinds=long",
            Some(KindFilter::Exclude(vec![PubkyAppPostKind::Long])),
        ),
    ] {
        assert_eq!(
            unfiltered_keys(None, kind, window).await?,
            window_keys,
            "{shape}"
        );
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
/// as the end of the feed: every page is full until the ranked posts run out,
/// however many hidden posts sit in between. Walked on the ranked set and on
/// the Cypher rule (`kind=short`).
#[tokio_shared_rt::test(shared)]
async fn test_all_timeline_pages_stay_full() -> Result<()> {
    for shape in ["", "&kind=short"] {
        let mut start = WINDOW_START;
        let mut served = Vec::new();
        for _ in 0..3 {
            let page = keys(&format!("{WINDOW}{shape}&start={start}&limit=1")).await?;
            let page_keys = post_keys_in(&page);
            assert_eq!(
                page_keys.len(),
                1,
                "a full page while ranked posts remain{shape}"
            );
            served.extend(page_keys);
            start = page["last_post_score"].as_i64().expect("cursor") - 1;
        }
        assert_eq!(served, ranked_window_keys(), "{shape}");

        // Only the hidden observer post is left below the cursor.
        let end = keys(&format!("{WINDOW}{shape}&start={start}&limit=1")).await?;
        assert!(post_keys_in(&end).is_empty(), "end of stream{shape}: {end}");
        assert!(end["last_post_score"].is_null());

        // A head poll: posts newer than a known head, down to `end`.
        let poll = keys(&format!(
            "source=all&sorting=timeline{shape}&start={WINDOW_START}&end=1650000000003&limit=10"
        ))
        .await?;
        assert_eq!(
            post_keys_in(&poll),
            ranked_window_keys()[..2].to_vec(),
            "{shape}"
        );
    }
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_all_timeline_ascending_and_skip_are_exact() -> Result<()> {
    let page = keys(&format!(
        "{WINDOW}&start={WINDOW_START}&order=ascending&limit=50"
    ))
    .await?;
    let mut ascending = ranked_window_keys();
    ascending.reverse();
    assert_eq!(post_keys_in(&page), ascending);

    let page = keys(&format!("{WINDOW}&start={WINDOW_START}&skip=1&limit=1")).await?;
    assert_eq!(post_keys_in(&page), ranked_window_keys()[1..2].to_vec());

    let page = keys(&format!("{WINDOW}&start={WINDOW_START}&skip=3&limit=1")).await?;
    assert!(post_keys_in(&page).is_empty(), "skip past the ranked posts");
    Ok(())
}

/// A single tag reads the label's ranked set: D2's tagged reply stays (tag
/// streams carry replies), the spammer's tagged reply goes.
#[tokio_shared_rt::test(shared)]
async fn test_single_tag_reads_the_ranked_set() -> Result<()> {
    let page = keys("source=all&sorting=timeline&tags=nudity").await?;
    assert_eq!(post_keys_in(&page), [format!("{WOT_D2}:WOTPOSTREPLY1")]);

    let page = keys("source=all&sorting=timeline&tags=wmtag1").await?;
    assert!(
        post_keys_in(&page).is_empty(),
        "spammer's reply served: {page}"
    );

    let page = posts("source=all&sorting=timeline&tags=wmtag1").await?;
    assert!(ids_in(&page).is_empty(), "spammer's reply served: {page}");
    Ok(())
}

/// A ranked viewer gets the same filtered stream as an anonymous request.
#[tokio_shared_rt::test(shared)]
async fn test_ranked_viewer_gets_the_filtered_stream() -> Result<()> {
    let viewer = format!("viewer_id={WOT_D1}");
    let page = keys(&format!("{WINDOW}&start={WINDOW_START}&limit=50&{viewer}")).await?;
    assert_eq!(post_keys_in(&page), ranked_window_keys());

    let page = keys(&format!(
        "{WINDOW}&start={WINDOW_START}&kind=short&limit=50&{viewer}"
    ))
    .await?;
    assert_eq!(post_keys_in(&page), ranked_window_keys());

    let page = keys(&format!("source=all&sorting=timeline&tags=wmtag1&{viewer}")).await?;
    assert!(post_keys_in(&page).is_empty());
    Ok(())
}

/// A viewer outside the ranking, or one Nexus does not know, gets exactly the
/// unfiltered stream on every shape, the Cypher ones included.
#[tokio_shared_rt::test(shared)]
async fn test_unranked_and_unknown_viewers_get_the_unfiltered_stream() -> Result<()> {
    let window = page_between(Some(WINDOW_START), Some(WINDOW_END), 50);
    let untagged = unfiltered_keys(None, None, window).await?;
    let short = unfiltered_keys(
        None,
        Some(KindFilter::Kind(PubkyAppPostKind::Short)),
        window,
    )
    .await?;
    // The route's default page size.
    let tagged = unfiltered_keys(Some(&["wmtag1"]), None, page_between(None, None, 10)).await?;
    let spammer_post = format!("{SPAMMER}:{SPAMMER_POST}");
    assert!(untagged.contains(&spammer_post), "fixture: {untagged:?}");
    assert!(short.contains(&spammer_post), "fixture: {short:?}");
    assert_eq!(tagged, [format!("{SPAMMER}:WOTPOSTMODF01")]);

    for viewer in [SPAMMER, UNKNOWN_USER] {
        let viewer = format!("viewer_id={viewer}");

        let page = keys(&format!("{WINDOW}&start={WINDOW_START}&limit=50&{viewer}")).await?;
        assert_eq!(post_keys_in(&page), untagged, "{viewer}");

        let page = posts(&format!("{WINDOW}&start={WINDOW_START}&limit=50&{viewer}")).await?;
        let expected: Vec<String> = untagged
            .iter()
            .map(|key| key.split_once(':').expect("author:post").1.to_string())
            .collect();
        assert_eq!(ids_in(&page), expected, "{viewer}");

        let page = keys(&format!(
            "{WINDOW}&start={WINDOW_START}&kind=short&limit=50&{viewer}"
        ))
        .await?;
        assert_eq!(post_keys_in(&page), short, "{viewer}");

        let page = keys(&format!("source=all&sorting=timeline&tags=wmtag1&{viewer}")).await?;
        assert_eq!(post_keys_in(&page), tagged, "{viewer}");
    }
    Ok(())
}

/// Every other source and sorting is untouched: the author stream and the
/// engagement sort still serve an unranked author's post.
#[tokio_shared_rt::test(shared)]
async fn test_out_of_scope_streams_are_unfiltered() -> Result<()> {
    let page = keys(&format!(
        "source=author&author_id={SPAMMER}&sorting=timeline&limit=50"
    ))
    .await?;
    assert!(
        post_keys_in(&page).contains(&format!("{SPAMMER}:{SPAMMER_POST}")),
        "author stream: {page}"
    );

    // The label that is empty on the timeline sort keeps the spammer's reply on
    // the engagement sort, which has no ranked sets.
    let page = keys("source=all&sorting=total_engagement&tags=wmtag1").await?;
    assert_eq!(
        post_keys_in(&page),
        [format!("{SPAMMER}:WOTPOSTMODF01")],
        "the engagement sort must keep unranked authors"
    );
    Ok(())
}
