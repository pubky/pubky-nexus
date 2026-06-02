use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;
use serde_json::Value;

use super::{KEYS_ROOT_PATH, ROOT_PATH};

// Web of Trust fixture (docker/test-graph/mocks/wot.cypher).
// Follows: O->D1, O->D1B, O->M ; D1->D2, D1B->D2 ; D1->O (cycle back to the
// observer). Spammer S has no inbound follows.
const OBSERVER: &str = "y6apowjmcg8rocmd9jirg95fyf3yykwuhqxozzts4mjipk4n7iao";
const SPAMMER: &str = "qdsygndnk45m9ru5jseg3uxk5xg4usj9hrcraqbzgigapzweaa9o";

const P_O: &str = "WOTPOSTO00001";
const P_D1: &str = "WOTPOSTD10002";
const P_D1B: &str = "WOTPOSTD1B003";
const P_D2: &str = "WOTPOSTD20004";
const P_S: &str = "WOTPOSTS00006";
const P_BTC1: &str = "WOTPOSTBTC1A";
const P_BTC2: &str = "WOTPOSTBTC2A";
const P_BTC3: &str = "WOTPOSTBTC3A";
const P_BTC4: &str = "WOTPOSTBTC4A";
const P_BTC5: &str = "WOTPOSTBTC5A";
const P_ART1: &str = "WOTPOSTART1A";

fn post_ids(body: &Value) -> Vec<String> {
    body.as_array()
        .expect("Post stream should be an array")
        .iter()
        .map(|p| {
            p["details"]["id"]
                .as_str()
                .expect("post should have a string id")
                .to_string()
        })
        .collect()
}

fn assert_excludes(body: &Value, excluded: &[&str]) {
    let ids = post_ids(body);
    for id in excluded {
        assert!(
            !ids.iter().any(|got| got == id),
            "post {id} should be excluded, got {ids:?}"
        );
    }
}

// Order-independent set comparison (WoT streams are graph-path, no positional guarantee).
fn assert_exact_set(body: &Value, expected: &[&str]) {
    let mut ids = post_ids(body);
    ids.sort();
    let mut want: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
    want.sort();
    assert_eq!(ids, want, "post id set mismatch");
}

fn count_id(body: &Value, id: &str) -> usize {
    post_ids(body).iter().filter(|got| *got == id).count()
}

#[tokio_shared_rt::test(shared)]
async fn test_wot_post_stream_excludes_spam_and_self() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=wot&observer_id={OBSERVER}&depth=2&limit=30");
    let body = get_request(&path).await?;
    assert!(body.is_array());
    // Spammer (no inbound follows), the observer's own post (excluded even though
    // the D1->O cycle makes O reachable at depth 2), and merely-tagged (not
    // followed) bitcoiners/artists are all absent from the trust network.
    assert_excludes(&body, &[P_S, P_O, P_BTC1, P_BTC5, P_ART1]);
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_wot_post_stream_depth_matrix_and_dedup() -> Result<()> {
    // depth 1: only directly-followed authors with posts (D1, D1B); D2 is at depth 2.
    let path = format!("{ROOT_PATH}?source=wot&observer_id={OBSERVER}&depth=1&limit=30");
    let body = get_request(&path).await?;
    assert_exact_set(&body, &[P_D1, P_D1B]);

    // depth 2: adds D2.
    let path = format!("{ROOT_PATH}?source=wot&observer_id={OBSERVER}&depth=2&limit=30");
    let body = get_request(&path).await?;
    assert_exact_set(&body, &[P_D1, P_D1B, P_D2]);
    // D2 is reachable via O->D1->D2 and O->D1B->D2 but must appear exactly once.
    assert_eq!(
        count_id(&body, P_D2),
        1,
        "P_D2 must dedup across follow paths"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_wot_domain_post_stream() -> Result<()> {
    // bitcoiner: 3 endorsed by the observer's WoT; BTC1 endorsed by two raters => once.
    let path = format!(
        "{ROOT_PATH}?source=wot_domain&observer_id={OBSERVER}&depth=2&domain_tags=bitcoiner&limit=30"
    );
    let body = get_request(&path).await?;
    assert_exact_set(&body, &[P_BTC1, P_BTC2, P_BTC3]);
    assert_eq!(
        count_id(&body, P_BTC1),
        1,
        "P_BTC1 must dedup across endorsers"
    );

    // btc-dev: only BTC4 (endorsed by depth-2 rater D2).
    let path = format!(
        "{ROOT_PATH}?source=wot_domain&observer_id={OBSERVER}&depth=2&domain_tags=btc-dev&limit=30"
    );
    let body = get_request(&path).await?;
    assert_exact_set(&body, &[P_BTC4]);

    // Union (OR over the label list).
    let path = format!(
        "{ROOT_PATH}?source=wot_domain&observer_id={OBSERVER}&depth=2&domain_tags=bitcoiner,btc-dev&limit=30"
    );
    let body = get_request(&path).await?;
    assert_exact_set(&body, &[P_BTC1, P_BTC2, P_BTC3, P_BTC4]);

    // artist is endorsed only by an out-of-WoT user => nothing for this observer.
    let path = format!(
        "{ROOT_PATH}?source=wot_domain&observer_id={OBSERVER}&depth=2&domain_tags=artist&limit=30"
    );
    let body = get_request(&path).await?;
    assert!(
        body.as_array().expect("array").is_empty(),
        "artist is not endorsed by the observer's WoT"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_wot_domain_with_tags_and_engagement_sorting() -> Result<()> {
    // Regression for the WHERE/AND ordering fix: wot_domain combined with a post
    // `tags` filter and TotalEngagement sorting must produce valid Cypher (200).
    let path = format!(
        "{ROOT_PATH}?source=wot_domain&observer_id={OBSERVER}&depth=2&domain_tags=bitcoiner&tags=opensource&sorting=total_engagement&limit=30"
    );
    let body = get_request(&path).await?;
    assert!(
        body.is_array(),
        "wot_domain + tags + total_engagement must be a valid query"
    );
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_wot_validation_errors() -> Result<()> {
    // wot_domain without domain_tags.
    let path = format!("{ROOT_PATH}?source=wot_domain&observer_id={OBSERVER}&depth=2");
    invalid_get_request(&path, StatusCode::BAD_REQUEST).await?;

    // wot without observer_id.
    let path = format!("{ROOT_PATH}?source=wot&depth=2");
    invalid_get_request(&path, StatusCode::BAD_REQUEST).await?;

    // wot_domain without observer_id.
    let path = format!("{ROOT_PATH}?source=wot_domain&domain_tags=bitcoiner&depth=2");
    invalid_get_request(&path, StatusCode::BAD_REQUEST).await?;

    // depth out of range.
    let path = format!("{ROOT_PATH}?source=wot&observer_id={OBSERVER}&depth=4");
    invalid_get_request(&path, StatusCode::BAD_REQUEST).await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_wot_cold_start_and_backward_compat() -> Result<()> {
    // Cold start: an observer with no follows yields an empty (but valid) stream.
    let path = format!("{ROOT_PATH}?source=wot&observer_id={SPAMMER}&depth=2");
    let body = get_request(&path).await?;
    assert!(
        body.as_array().expect("array").is_empty(),
        "cold-start WoT stream should be empty"
    );

    // Backward-compat: `depth` on a non-wot source is ignored; global feed still works.
    let path = format!("{ROOT_PATH}?source=all&depth=2&limit=5");
    let body = get_request(&path).await?;
    assert!(
        body.is_array(),
        "source=all with depth should still return the global feed"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_wot_post_keys_parity() -> Result<()> {
    let params = format!("source=wot&observer_id={OBSERVER}&depth=2&limit=30");
    let keys_body = get_request(&format!("{KEYS_ROOT_PATH}?{params}")).await?;
    let posts_body = get_request(&format!("{ROOT_PATH}?{params}")).await?;

    let keys = keys_body["post_keys"]
        .as_array()
        .expect("post key stream should expose a post_keys array");
    let posts = posts_body
        .as_array()
        .expect("post stream should be an array");

    assert_eq!(
        keys.len(),
        posts.len(),
        "WoT post-keys and posts streams must align"
    );
    assert_eq!(posts.len(), 3, "observer O has 3 WoT posts at depth 2");
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_wot_post_stream_order_ascending() -> Result<()> {
    // Graph-backed WoT stream honors `order`: ascending is oldest-first by indexed_at.
    let path =
        format!("{ROOT_PATH}?source=wot&observer_id={OBSERVER}&depth=2&order=ascending&limit=30");
    let body = get_request(&path).await?;
    let ids = post_ids(&body);
    assert_eq!(
        ids.iter().map(String::as_str).collect::<Vec<_>>(),
        vec![P_D1, P_D1B, P_D2],
        "order=ascending must return oldest-first"
    );
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_wot_post_keys_total_engagement_cursor() -> Result<()> {
    // The key-stream cursor must be the engagement score for `total_engagement`, not a
    // post timestamp (regression for the graph-path cursor).
    let path = format!(
        "{KEYS_ROOT_PATH}?source=wot&observer_id={OBSERVER}&depth=2&sorting=total_engagement&limit=30"
    );
    let body = get_request(&path).await?;
    let score = body["last_post_score"]
        .as_u64()
        .expect("last_post_score should be set for a non-empty stream");
    assert!(
        score < 1_000,
        "total_engagement cursor must be an engagement count, not a timestamp, got {score}"
    );
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_wot_post_stream_ascending_pagination_moves_forward() -> Result<()> {
    // Ascending pages forward from the `start` cursor: with `start` at D1B's
    // timestamp the page holds D1B and the newer D2, never the older D1
    // (regression for the order-aware cursor bound).
    let start = 1_650_000_000_003u64; // P_D1B indexed_at
    let path = format!(
        "{ROOT_PATH}?source=wot&observer_id={OBSERVER}&depth=2&order=ascending&start={start}&limit=30"
    );
    let body = get_request(&path).await?;
    let ids = post_ids(&body);
    assert!(
        ids.iter().any(|id| id == P_D1B),
        "ascending page should include the cursor item D1B, got {ids:?}"
    );
    assert!(
        ids.iter().any(|id| id == P_D2),
        "ascending page should include D2 (newer than the cursor), got {ids:?}"
    );
    assert!(
        !ids.iter().any(|id| id == P_D1),
        "ascending page must not include D1 (older than the cursor), got {ids:?}"
    );
    Ok(())
}
