//! `source=starter_pack`: people to follow for a set of interest tags.
//!
//! Fixtures are `wot.cypher`, where taggers carry trust (D1 0.4, D1B 0.1, D2 0.2) and SPAMMER
//! has none. Ties break on endorser count then id, so `bitcoiner` is exactly:
//!
//! | candidate | endorsers | trust |
//! |-----------|-----------|-------|
//! | BTC1      | D1 + D1B  | 0.5   |
//! | BTC2      | D1        | 0.4   |
//! | OBSERVER  | D1        | 0.4   |
//! | BTC3      | D1B       | 0.1   |
//! | SPAMMER   | itself    | 0.0   |
//! | BTC5      | SPAMMER   | 0.0   |

use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;
use nexus_webapi::models::ErrorResponsePayload;
use serde_json::Value;

const OBSERVER: &str = "y6apowjmcg8rocmd9jirg95fyf3yykwuhqxozzts4mjipk4n7iao";
const D1: &str = "qjftuwjog819ki1wktuy5tndebce36bmxxwtjjm3z1fr97jk9yuo";
const D2: &str = "smf4xrqfhx7stnufkjzhbjyu3rbgb3gga64srqmzcyyoyzefse9y";
const SPAMMER: &str = "qdsygndnk45m9ru5jseg3uxk5xg4usj9hrcraqbzgigapzweaa9o";
const BTC1: &str = "tfqnfppxtr8xei6n3zrfa1b7mmc6gqn41szgutgcccea33pqf3yo";
const BTC2: &str = "uuxjusor98rw4xdo3k3shsgdqwi844si14aaxfcnjxyczjt5eqxy";
const BTC3: &str = "qwzn6jx1gm1ziptn41dxonqy1rpuumwggdq1hu6zc334qep3kjho";
const BTC4: &str = "z5eect18reuccuwuq78da8k5re3y8si346n3bah45gad6t6b1zby";
const BTC5: &str = "wbhcz1gfz14jc4qjg74auyo5bwxd4gc3y84ic18iro17yi4bgz3y";

async fn pack(query: &str) -> Result<Vec<String>> {
    let res = get_request(&format!("/v0/stream/users?source=starter_pack&{query}")).await?;
    let users = res.as_array().expect("starter pack should be an array");
    Ok(users
        .iter()
        .map(|u| {
            u["details"]["id"]
                .as_str()
                .expect("every entry should carry an id")
                .to_string()
        })
        .collect())
}

async fn bad_request(query: &str) -> Result<String> {
    let res = invalid_get_request(
        &format!("/v0/stream/users?{query}"),
        StatusCode::BAD_REQUEST,
    )
    .await?;
    let payload: ErrorResponsePayload =
        serde_json::from_value(res).expect("Response should be a valid ErrorResponsePayload");
    Ok(payload.error)
}

#[tokio_shared_rt::test(shared)]
async fn test_starter_pack_ranks_by_summed_endorser_trust() -> Result<()> {
    let ids = pack("tags=bitcoiner&limit=20").await?;

    assert_eq!(
        ids,
        [BTC1, BTC2, OBSERVER, BTC3, SPAMMER, BTC5],
        "candidates should rank by summed endorser trust, then endorser count, then id"
    );

    Ok(())
}

/// Counting heads instead of trust would tie BTC2 and BTC3 and put BTC3 first on id.
#[tokio_shared_rt::test(shared)]
async fn test_starter_pack_prefers_a_trusted_endorser_over_a_weak_one() -> Result<()> {
    let ids = pack("tags=bitcoiner&limit=20").await?;
    let rank = |id: &str| ids.iter().position(|c| c == id).expect("candidate present");

    assert!(
        rank(BTC2) < rank(BTC3),
        "a 0.4-trust endorsement should outrank a 0.1-trust one, got {ids:?}"
    );
    assert!(
        rank(BTC3) < rank(BTC5),
        "any trusted endorsement should outrank one from a rater with no score, got {ids:?}"
    );

    Ok(())
}

/// A self-tag is worth its author's own trust, and SPAMMER has none.
#[tokio_shared_rt::test(shared)]
async fn test_starter_pack_does_not_let_a_self_tag_mint_reputation() -> Result<()> {
    let ids = pack("tags=bitcoiner&limit=20").await?;

    let rank = |id: &str| ids.iter().position(|c| c == id).expect("candidate present");

    assert!(ids.contains(&SPAMMER.to_string()), "self-tags still count");
    assert!(
        rank(SPAMMER) > rank(BTC3),
        "an untrusted self-tag should rank below every trusted endorsement, got {ids:?}"
    );

    Ok(())
}

/// No profile carries `wotreview`, so only the post arm can find this candidate.
#[tokio_shared_rt::test(shared)]
async fn test_starter_pack_finds_authors_of_tagged_posts() -> Result<()> {
    let ids = pack("tags=wotreview&limit=20").await?;

    assert_eq!(
        ids,
        [D2],
        "the author of the tagged post should be the pack"
    );

    Ok(())
}

/// A flat ranking would spend both slots on `bitcoiner`, which has six candidates to one.
#[tokio_shared_rt::test(shared)]
async fn test_starter_pack_gives_a_niche_label_a_slot() -> Result<()> {
    let ids = pack("tags=bitcoiner,btc-dev&limit=2").await?;

    assert_eq!(
        ids,
        [BTC1, BTC4],
        "each label should place its top candidate"
    );

    Ok(())
}

/// These two share their only candidate; disjoint labels could not catch a dedup regression.
#[tokio_shared_rt::test(shared)]
async fn test_starter_pack_returns_each_user_once() -> Result<()> {
    let ids = pack("tags=btc-dev,packoverlap&limit=20").await?;

    assert_eq!(ids, [BTC4], "a shared candidate should appear once");

    Ok(())
}

/// The candidate is endorsed and has a post, so only the deleted check can remove it.
#[tokio_shared_rt::test(shared)]
async fn test_starter_pack_excludes_deleted_users() -> Result<()> {
    let ids = pack("tags=packdeleted&limit=20").await?;

    assert!(
        ids.is_empty(),
        "a deleted account should never reach an onboarding pack, got {ids:?}"
    );

    Ok(())
}

/// The default window still asks that a candidate has posted at all.
#[tokio_shared_rt::test(shared)]
async fn test_starter_pack_excludes_accounts_that_never_posted() -> Result<()> {
    let ids = pack("tags=satoshi&limit=20").await?;

    assert!(
        ids.is_empty(),
        "an account with no posts should not be recommended, got {ids:?}"
    );

    Ok(())
}

/// Pins the fallback that ships until the trust job runs, where every score is null.
#[tokio_shared_rt::test(shared)]
async fn test_starter_pack_orders_unscored_endorsements_deterministically() -> Result<()> {
    let ids = pack("tags=bitcoiner&limit=20").await?;
    let rank = |id: &str| ids.iter().position(|c| c == id).expect("candidate present");

    assert!(
        rank(SPAMMER) < rank(BTC5),
        "candidates with no endorser trust should fall back to id order, got {ids:?}"
    );

    Ok(())
}

/// A repeat must not buy a label two turns in the round-robin.
#[tokio_shared_rt::test(shared)]
async fn test_starter_pack_collapses_repeated_labels() -> Result<()> {
    let once = pack("tags=bitcoiner&limit=20").await?;
    let twice = pack("tags=bitcoiner,BITCOINER&limit=20").await?;

    assert_eq!(once, twice, "a repeated label should not change the pack");

    Ok(())
}

/// The cold-start case this endpoint exists for: no viewer, no follows, no history.
#[tokio_shared_rt::test(shared)]
async fn test_starter_pack_works_with_no_viewer() -> Result<()> {
    let res = get_request("/v0/stream/users?source=starter_pack&tags=bitcoiner&limit=20").await?;
    let users = res.as_array().expect("starter pack should be an array");

    assert!(
        !users.is_empty(),
        "a viewerless pack should still return people"
    );
    for user in users {
        assert!(
            user["details"]["name"].is_string(),
            "Name should be a string"
        );
        assert!(
            user["counts"]["followers"].is_number(),
            "Counts should hydrate"
        );
        assert_eq!(
            user["relationship"]["following"],
            Value::Bool(false),
            "with no viewer the relationship flags default to false"
        );
    }

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_starter_pack_excludes_the_user_and_who_they_follow() -> Result<()> {
    // D1 follows D2, the only `wotreview` candidate.
    let ids = pack(&format!("tags=wotreview&limit=20&user_id={D1}")).await?;
    assert!(
        ids.is_empty(),
        "already-followed users should be dropped, got {ids:?}"
    );

    // viewer_id is how a client identifies itself everywhere else, so it must exclude here.
    let ids = pack(&format!("tags=wotreview&limit=20&viewer_id={D1}")).await?;
    assert!(
        ids.is_empty(),
        "viewer_id alone should exclude followed users, got {ids:?}"
    );

    let ids = pack(&format!("tags=bitcoiner&limit=20&user_id={BTC1}")).await?;
    assert!(
        !ids.contains(&BTC1.to_string()),
        "the user should not be in their own pack"
    );
    assert!(
        ids.contains(&BTC2.to_string()),
        "everyone else should survive"
    );

    Ok(())
}

/// Every fixture post is from 2022, so a recent window empties the pack.
#[tokio_shared_rt::test(shared)]
async fn test_starter_pack_timeframe_gates_on_activity() -> Result<()> {
    let ungated = pack("tags=bitcoiner&limit=20").await?;
    assert!(
        !ungated.is_empty(),
        "the default window should not gate on recency"
    );

    let gated = pack("tags=bitcoiner&limit=20&timeframe=this_month").await?;
    assert!(
        gated.is_empty(),
        "stale accounts should drop out of a recent window, got {gated:?}"
    );

    Ok(())
}

/// Anyone can put a moderation label on a profile; only the moderator's own tags are deleted.
#[tokio_shared_rt::test(shared)]
async fn test_starter_pack_rejects_moderation_labels() -> Result<()> {
    let error = bad_request("source=starter_pack&tags=hatespeech").await?;
    assert!(
        error.contains("hatespeech") && error.contains("moderation label"),
        "error should name the rejected label and why, got: {error}"
    );

    let error = bad_request("source=starter_pack&tags=bitcoiner,violence").await?;
    assert!(
        error.contains("violence"),
        "one bad label should fail the request, got: {error}"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_starter_pack_requires_tags() -> Result<()> {
    let error = bad_request("source=starter_pack&limit=20").await?;
    assert!(
        error.contains("tags query param must be provided"),
        "error should say tags is required, got: {error}"
    );

    Ok(())
}

/// Silently ignoring the filter would return a plausible stream answering a different question.
#[tokio_shared_rt::test(shared)]
async fn test_starter_pack_tags_are_rejected_on_other_sources() -> Result<()> {
    let error = bad_request("source=most_followed&tags=bitcoiner").await?;
    assert!(
        error.contains("only supported for source 'starter_pack'"),
        "error should explain tags is starter_pack only, got: {error}"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_starter_pack_caps_the_label_count() -> Result<()> {
    let error = bad_request("source=starter_pack&tags=a,b,c,d,e,f").await?;
    assert!(
        error.contains("Maximum 5"),
        "error should name the label cap rather than any other rejection, got: {error}"
    );

    Ok(())
}

/// user_id is the subject and viewer_id only shapes relationship flags, as on every other source.
#[tokio_shared_rt::test(shared)]
async fn test_starter_pack_user_id_outranks_viewer_id() -> Result<()> {
    // D1 follows D2, the only `wotreview` candidate. BTC1 follows nobody.
    let ids = pack(&format!(
        "tags=wotreview&limit=20&user_id={D1}&viewer_id={BTC1}"
    ))
    .await?;
    assert!(
        ids.is_empty(),
        "user_id should drive exclusion, got {ids:?}"
    );

    let ids = pack(&format!(
        "tags=wotreview&limit=20&user_id={BTC1}&viewer_id={D1}"
    ))
    .await?;
    assert_eq!(
        ids,
        [D2],
        "viewer_id should not exclude once user_id is given"
    );

    Ok(())
}

/// Past this the query cannot fetch enough candidates, so it must reject rather than return a
/// short page.
#[tokio_shared_rt::test(shared)]
async fn test_starter_pack_rejects_unpageable_skip() -> Result<()> {
    let error = bad_request("source=starter_pack&tags=bitcoiner&skip=101").await?;
    assert!(
        error.contains("skip must be at most 100"),
        "error should name the skip bound, got: {error}"
    );

    pack("tags=bitcoiner&skip=100&limit=20").await?;

    Ok(())
}

/// Two labels, so the page is a window on the interleaved list rather than on one ranking.
#[tokio_shared_rt::test(shared)]
async fn test_starter_pack_paginates_over_the_merged_list() -> Result<()> {
    let all = pack("tags=bitcoiner,btc-dev&limit=20").await?;
    let page = pack("tags=bitcoiner,btc-dev&skip=1&limit=2").await?;

    assert_eq!(
        page,
        all[1..3],
        "a page should be a window on the same merged ranking"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_starter_pack_unknown_label_is_an_empty_pack() -> Result<()> {
    let ids = pack("tags=nobodyusesthis&limit=20").await?;

    assert!(
        ids.is_empty(),
        "an unused label should return no one, got {ids:?}"
    );

    Ok(())
}
