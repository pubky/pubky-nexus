use super::utils::{analyse_tag_details_structure, compare_tag_details, TagMockup};
use crate::utils::server::TestServiceServer;
use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;
use deadpool_redis::redis::AsyncCommands;
use nexus_common::db::get_redis_conn;
use nexus_common::models::tag::TagDetails;
use nexus_common::types::DynError;
use nexus_webapi::routes::v0::TaggersInfoResponse;
use serde_json::Value;

// ##### WoT user tags ####
// We need to run all the test sequentially, if not there might be some inconsistency when we make concurrently the requests
// The first request writes the cache, others read from cache

const AURELIO_USER: &str = "c4yotzcb76d31y44jsymtdcowqg7oyqej46jty3yy7ybtzt9x41o";
const EPICTTO_VIEWER: &str = "bbkdkhm97pytrb785rdpornkjpcxi331hpq446ckn6rhb4abiguy";
const NOW_TAG: &str = "now";
const ATHENS_TAG: &str = "athens";

const USER_A: &str = "cjoodgkwaf1bwepoe8m6zsp8guobh5wdwmqqnk496jcd175jjwey";
const USER_B: &str = "fs8qf51odhpf9ecoms8i9tbjtyshhjdejpsf3nxcbup3ugs7q4xo";
const USER_C: &str = "cuimec4ngawamq8wa6fjzki6boxmwqcm11x6g7ontufrjwgdaxqo";

// WARNING: To test that integration test, the Cache:... indexes
// related with WoT has to be deleted
#[tokio_shared_rt::test(shared)]
async fn test_wot_user_tags_endpoints() -> Result<(), DynError> {
    let _ = clear_wot_tags_cache().await;

    // Make sure, we still not index the WoT tags requesting the taggers
    let path =
        format!("/v0/user/{AURELIO_USER}/taggers/{ATHENS_TAG}?viewer_id={EPICTTO_VIEWER}&depth=2");
    // If we get error here, delete the Cache:... indexes
    // Before indexing, the taggers endpoint should return empty list
    let body = get_request(&path).await?;
    let taggers_info: TaggersInfoResponse = serde_json::from_value(body)?;
    assert!(taggers_info.users.is_empty());

    // => Start indexing the WoT tags
    let path = format!("/v0/user/{AURELIO_USER}/tags?viewer_id={EPICTTO_VIEWER}&depth=2");
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 2);

    // Validate that the posts tag structure
    analyse_tag_details_structure(tags);

    // Analyse the tag that is in the 4th index
    let now_hot_tag = TagMockup::new(String::from(NOW_TAG), 2, 2);
    let athens_hot_tag = TagMockup::new(String::from(ATHENS_TAG), 3, 3);
    compare_tag_details(&tags[1], now_hot_tag);
    compare_tag_details(&tags[0], athens_hot_tag);

    // => test_wot_user_tags_endpoint_with_tag_limit
    let path =
        format!("/v0/user/{AURELIO_USER}/tags?viewer_id={EPICTTO_VIEWER}&depth=2&limit_tags=1");
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 1);

    // Validate that the posts tag structure
    analyse_tag_details_structure(tags);

    // The hottest tag (athens) sorts first
    let athens_hot_tag = TagMockup::new(String::from(ATHENS_TAG), 3, 3);
    compare_tag_details(&tags[0], athens_hot_tag);

    // => test_wot_user_tags_endpoint_with_tag_skip
    let path =
        format!("/v0/user/{AURELIO_USER}/tags?viewer_id={EPICTTO_VIEWER}&depth=2&skip_tags=1");
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 1);

    // Validate that the posts tag structure
    analyse_tag_details_structure(tags);

    // Analyse the tag that is in the 1st index
    let now_hot_tag = TagMockup::new(String::from(NOW_TAG), 2, 2);
    compare_tag_details(&tags[0], now_hot_tag);

    // => test_wot_user_tags_endpoint_with_tag_skip_and_taggers_limit
    let path = format!(
        "/v0/user/{AURELIO_USER}/tags?viewer_id={EPICTTO_VIEWER}&depth=2&skip_tags=1&limit_taggers=1"
    );
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 1);

    // Validate that the posts tag structure
    analyse_tag_details_structure(tags);

    // Analyse the tag that is in the 1st index
    let now_hot_tag = TagMockup::new(String::from(NOW_TAG), 1, 2);
    compare_tag_details(&tags[0], now_hot_tag);

    // => test_wot_user_tags_endpoint_with_tagger_limit
    let path =
        format!("/v0/user/{AURELIO_USER}/tags?viewer_id={EPICTTO_VIEWER}&depth=2&limit_taggers=1");
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 2);

    // Validate that the posts tag structure
    analyse_tag_details_structure(tags);

    // // Analyse the tag that is in the 4th index
    let now_hot_tag = TagMockup::new(String::from(NOW_TAG), 1, 2);
    let athens_hot_tag = TagMockup::new(String::from(ATHENS_TAG), 1, 3);
    compare_tag_details(&tags[1], now_hot_tag);
    compare_tag_details(&tags[0], athens_hot_tag);

    // => test_wot_user_tags_endpoint_with_tag_and_tagger_limit
    let path = format!(
        "/v0/user/{AURELIO_USER}/tags?viewer_id={EPICTTO_VIEWER}&depth=2&limit_tags=1&limit_taggers=1"
    );
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 1);

    // Validate that the posts tag structure
    analyse_tag_details_structure(tags);

    // // Analyse the tag that is in the 4th index
    let now_hot_tag = TagMockup::new(String::from(ATHENS_TAG), 1, 3);
    compare_tag_details(&tags[0], now_hot_tag);

    // => test_wot_user_label_taggers
    let path =
        format!("/v0/user/{AURELIO_USER}/taggers/{ATHENS_TAG}?viewer_id={EPICTTO_VIEWER}&depth=2");
    let body = get_request(&path).await?;

    let mut mock_taggers = vec![USER_A, USER_B, USER_C];
    verify_taggers_list(mock_taggers, body);

    // => test_wot_user_label_taggers_with_limit
    let path = format!(
        "/v0/user/{AURELIO_USER}/taggers/{ATHENS_TAG}?viewer_id={EPICTTO_VIEWER}&depth=2&limit=2"
    );
    let body = get_request(&path).await?;

    mock_taggers = vec![USER_A, USER_B];
    verify_taggers_list(mock_taggers, body);

    // => test_wot_user_label_taggers_with_limit
    let path = format!(
        "/v0/user/{AURELIO_USER}/taggers/{ATHENS_TAG}?viewer_id={EPICTTO_VIEWER}&depth=2&limit=1&skip=1"
    );
    let body = get_request(&path).await?;

    mock_taggers = vec![USER_B];
    verify_taggers_list(mock_taggers, body);

    // USER VIEW
    let path = format!("/v0/user/{AURELIO_USER}?viewer_id={EPICTTO_VIEWER}&depth=2");
    let body = get_request(&path).await?;
    let tags = body["tags"].clone();

    mock_taggers = vec![USER_A, USER_B, USER_C];
    verify_user_taggers(mock_taggers, tags[0].clone(), String::from(ATHENS_TAG));

    mock_taggers = vec![USER_A, USER_C];
    verify_user_taggers(mock_taggers, tags[1].clone(), String::from(NOW_TAG));

    // USER STREAM
    // TODO: MIssing that integration test, user_id-source-depth. Add more mock for that test

    Ok(())
}

fn verify_taggers_list(mock_taggers: Vec<&str>, body: Value) {
    let taggers_info: TaggersInfoResponse = serde_json::from_value(body).unwrap();
    assert_eq!(taggers_info.users.len(), mock_taggers.len());

    assert!(
        !taggers_info.users.is_empty(),
        "Post stream should not be empty"
    );
    assert_eq!(
        taggers_info.users.len(),
        mock_taggers.len(),
        "The endpoint result has to have the same lenght as mock data"
    );

    for (index, user_id) in taggers_info.users.iter().enumerate() {
        assert_eq!(
            mock_taggers[index], user_id,
            "The post ids should be the same"
        );
    }
}

fn verify_user_taggers(mock_taggers: Vec<&str>, tag_details: Value, tag: String) {
    let tag_details: TagDetails = serde_json::from_value(tag_details).unwrap();

    assert_eq!(
        tag_details.taggers_count,
        mock_taggers.len(),
        "The endpoint result has to have the same lenght as mock data"
    );

    assert_eq!(tag, tag_details.label, "The labels does not match");

    for (index, user_id) in tag_details.taggers.iter().enumerate() {
        assert_eq!(
            mock_taggers[index], user_id,
            "The post ids should be the same"
        );
    }
}

async fn clear_wot_tags_cache() -> Result<(), DynError> {
    // Ensure the server is running, for redis connection
    TestServiceServer::get_test_server().await;
    let mut redis_conn = get_redis_conn().await?;

    let athens_key = format!("Cache:User:Taggers:{EPICTTO_VIEWER}:{AURELIO_USER}:{ATHENS_TAG}");
    let now_key = format!("Cache:User:Taggers:{EPICTTO_VIEWER}:{AURELIO_USER}:{NOW_TAG}");
    // Remove the SETs
    let _: () = redis_conn.del(athens_key).await?;
    let _: () = redis_conn.del(now_key).await?;
    // Remove the SORTED SET
    let sorted_set_key = format!("Cache:Sorted:Users:Tag:{EPICTTO_VIEWER}:{AURELIO_USER}");
    let _: () = redis_conn.del(sorted_set_key).await?;
    Ok(())
}

// ##### WoT post tags (moderation-bot visibility) #####
// The post-tag WoT path is graph-only (no cache), so no cache-clear is needed.
// Fixture (docker/test-graph/mocks/wot.cypher): mod bot M tags reply WOTPOSTREPLY1
// (authored by D2) with `nudity`; observer O follows M, spammer S does not.
const WOT_OBSERVER: &str = "y6apowjmcg8rocmd9jirg95fyf3yykwuhqxozzts4mjipk4n7iao";
const WOT_REPLY_AUTHOR: &str = "smf4xrqfhx7stnufkjzhbjyu3rbgb3gga64srqmzcyyoyzefse9y";
const WOT_NON_FOLLOWER: &str = "qdsygndnk45m9ru5jseg3uxk5xg4usj9hrcraqbzgigapzweaa9o";
const WOT_REPLY_POST: &str = "WOTPOSTREPLY1";
const NUDITY_TAG: &str = "nudity";
// D2's parent post: D2 is in the observer's WoT, but no WoT member tagged this post,
// so its WoT post-tag list is empty (existing post, no trusted tags). 13-char PostId.
const WOT_UNTAGGED_POST: &str = "WOTPOSTD20004";
// Deep reply tagged by WoT members D1/D1B ('wotreview') and M ('wotflag'); used to
// check that WoT post-tags honor skip_tags/limit_tags/limit_taggers like the global view.
const WOT_TAG_LIMIT_POST: &str = "WOTPOSTTAGS01";

#[tokio_shared_rt::test(shared)]
async fn test_wot_post_tags_mod_visibility() -> Result<(), DynError> {
    // The observer follows the mod bot, so its `nudity` tag on the post is visible.
    let path = format!(
        "/v0/post/{WOT_REPLY_AUTHOR}/{WOT_REPLY_POST}/tags?viewer_id={WOT_OBSERVER}&depth=2"
    );
    let body = get_request(&path).await?;
    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(
        tags.len(),
        1,
        "the mod bot's tag should surface for a follower"
    );
    assert_eq!(tags[0]["label"], NUDITY_TAG);

    // A viewer who does not follow the mod bot: the post exists but its WoT has no
    // tags on it, so the response is an empty list (200), not 404.
    let path = format!(
        "/v0/post/{WOT_REPLY_AUTHOR}/{WOT_REPLY_POST}/tags?viewer_id={WOT_NON_FOLLOWER}&depth=2"
    );
    let body = get_request(&path).await?;
    let tags = body.as_array().expect("Tag list should be an array");
    assert!(
        tags.is_empty(),
        "a non-follower should not see the mod bot's tag"
    );

    // The observer's WoT is non-empty (follows D1/D1B/M) but none of them tagged this
    // post: it is a valid post, so the response is an empty list (200), not a 404.
    let path = format!(
        "/v0/post/{WOT_REPLY_AUTHOR}/{WOT_UNTAGGED_POST}/tags?viewer_id={WOT_OBSERVER}&depth=2"
    );
    let body = get_request(&path).await?;
    let tags = body.as_array().expect("Tag list should be an array");
    assert!(
        tags.is_empty(),
        "an existing post with no WoT-visible tags should be 200 [], not 404"
    );

    // `viewer_id` without `depth` is the global view, not WoT: the same spammer who
    // saw nothing with depth=2 now sees the mod bot's tag through the global path.
    let path =
        format!("/v0/post/{WOT_REPLY_AUTHOR}/{WOT_REPLY_POST}/tags?viewer_id={WOT_NON_FOLLOWER}");
    let body = get_request(&path).await?;
    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(
        tags.len(),
        1,
        "viewer_id without depth returns the global view"
    );
    assert_eq!(tags[0]["label"], NUDITY_TAG);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_wot_tag_depth_out_of_range_rejected() -> Result<(), DynError> {
    // An out-of-range WoT depth supplied with a viewer is a 400 on both tag endpoints,
    // consistent with the stream endpoints (rather than silently falling back to global).
    let path = format!(
        "/v0/post/{WOT_REPLY_AUTHOR}/{WOT_REPLY_POST}/tags?viewer_id={WOT_OBSERVER}&depth=4"
    );
    invalid_get_request(&path, StatusCode::BAD_REQUEST).await?;

    let path = format!("/v0/user/{AURELIO_USER}/tags?viewer_id={EPICTTO_VIEWER}&depth=4");
    invalid_get_request(&path, StatusCode::BAD_REQUEST).await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_wot_tag_depth_without_viewer_rejected() -> Result<(), DynError> {
    // `depth` only makes sense with a viewer; supplying it alone is a malformed WoT
    // request and is rejected with 400 (not silently treated as the global view).
    let path = format!("/v0/post/{WOT_REPLY_AUTHOR}/{WOT_REPLY_POST}/tags?depth=2");
    invalid_get_request(&path, StatusCode::BAD_REQUEST).await?;

    let path = format!("/v0/user/{AURELIO_USER}/tags?depth=2");
    invalid_get_request(&path, StatusCode::BAD_REQUEST).await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_wot_post_tags_respect_limits() -> Result<(), DynError> {
    // WOTPOSTTAGS01 has two WoT labels: wotreview (2 trusted taggers, D1/D1B) and
    // wotflag (1, M). Labels sort by tagger count; the limits bound the response.
    let base = format!(
        "/v0/post/{WOT_REPLY_AUTHOR}/{WOT_TAG_LIMIT_POST}/tags?viewer_id={WOT_OBSERVER}&depth=2"
    );
    let body = get_request(&base).await?;
    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 2, "both WoT labels are visible by default");
    assert_eq!(
        tags[0]["label"], "wotreview",
        "the label with more taggers sorts first"
    );
    assert_eq!(
        tags[0]["taggers"].as_array().expect("taggers array").len(),
        2,
        "wotreview has two trusted taggers"
    );

    // limit_tags caps the number of labels returned.
    let body = get_request(&format!("{base}&limit_tags=1")).await?;
    assert_eq!(
        body.as_array().expect("array").len(),
        1,
        "limit_tags=1 returns only the top label"
    );

    // limit_taggers caps the taggers within each label.
    let body = get_request(&format!("{base}&limit_taggers=1")).await?;
    assert_eq!(
        body.as_array().expect("array")[0]["taggers"]
            .as_array()
            .expect("taggers")
            .len(),
        1,
        "limit_taggers=1 caps wotreview's taggers"
    );
    Ok(())
}
