//! The viewer's tag on tag details and taggers (`relationship` and `tag_uri`): whether they
//! tagged the label, and the address of their tag file.
//! Fixture: docker/test-graph/mocks/tag-relationship.cypher. The viewer's tags
//! live in `mapky/tags/`, so the expected address can't be built from ids.

use anyhow::Result;
use deadpool_redis::redis::AsyncCommands;
use nexus_common::db::get_redis_conn;
use serde_json::{json, Value};

use crate::utils::server::TestServiceServer;
use crate::utils::{get_request, post_request};

const VIEWER: &str = "xcb38btb3ioaoxtinmwtmizfum1akeg5yajju67o6rrcxq47n76o";
const AUTHOR: &str = "zxtmja5wju4pi5s681i1xrr957tz1n6o5g6489erm7usiok1jmto";
const TAGGED_USER: &str = "xcnswkwriypnzekfuwed6h5xecinwjj46x3ao7ffppy61z6hafiy";
/// Read only by the tests that make the index disagree with the graph, which write its Redis keys
const DRIFT_USER: &str = "uq1swy84jaw6pmqi399oqcf6izs86peamftbfhcxjhzywu97x8wo";
const POST: &str = "2ZRT8G4VXQ0M0";
const RESOURCE: &str = "7a6e1c0de5f0a11ce0ffee0000000001";

const VIEWER_POST_TAG_URI: &str =
    "pubky://xcb38btb3ioaoxtinmwtmizfum1akeg5yajju67o6rrcxq47n76o/pub/mapky/tags/TRELPOSTV01";
const VIEWER_USER_TAG_URI: &str =
    "pubky://xcb38btb3ioaoxtinmwtmizfum1akeg5yajju67o6rrcxq47n76o/pub/mapky/tags/TRELUSERV01";
const VIEWER_DRIFT_USER_TAG_URI: &str =
    "pubky://xcb38btb3ioaoxtinmwtmizfum1akeg5yajju67o6rrcxq47n76o/pub/mapky/tags/TRELDRIFTV01";
const VIEWER_DRIFT_UNINDEXED_TAG_URI: &str =
    "pubky://xcb38btb3ioaoxtinmwtmizfum1akeg5yajju67o6rrcxq47n76o/pub/mapky/tags/TRELDRIFTV03";
const VIEWER_RESOURCE_TAG_URI: &str =
    "pubky://xcb38btb3ioaoxtinmwtmizfum1akeg5yajju67o6rrcxq47n76o/pub/mapky/tags/TRELRESV01";

/// `(relationship, tag_uri)` of a tag details entry or a taggers response. Both fields are
/// always present.
fn viewer_tag_fields(body: &Value) -> (Value, Value) {
    let field = |name: &str| {
        body.get(name)
            .cloned()
            .unwrap_or_else(|| panic!("no `{name}` in {body}"))
    };
    (field("relationship"), field("tag_uri"))
}

/// `(relationship, tag_uri)` of the tag with `label` in a tag details list.
fn viewer_tag(tags: &Value, label: &str) -> (Value, Value) {
    let tag = tags
        .as_array()
        .expect("tags should be an array")
        .iter()
        .find(|t| t["label"] == label)
        .unwrap_or_else(|| panic!("no tag with label {label}"));
    viewer_tag_fields(tag)
}

/// `(relationship, tag_uri)` of a label the viewer tagged with the tag file at `uri`.
fn tagged(uri: &str) -> (Value, Value) {
    (Value::Bool(true), Value::from(uri))
}

/// `(relationship, tag_uri)` of a label the viewer hasn't tagged.
fn untagged() -> (Value, Value) {
    (Value::Bool(false), Value::Null)
}

#[tokio_shared_rt::test(shared)]
async fn test_post_tags_carry_viewer_tag_uri() -> Result<()> {
    let tags = get_request(&format!("/v0/post/{AUTHOR}/{POST}/tags?viewer_id={VIEWER}")).await?;

    assert_eq!(
        viewer_tag(&tags, "trel-shared"),
        tagged(VIEWER_POST_TAG_URI)
    );
    assert_eq!(viewer_tag(&tags, "trel-other"), untagged());

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_tags_carry_viewer_tag_uri() -> Result<()> {
    let tags = get_request(&format!("/v0/user/{TAGGED_USER}/tags?viewer_id={VIEWER}")).await?;

    assert_eq!(
        viewer_tag(&tags, "trel-shared"),
        tagged(VIEWER_USER_TAG_URI)
    );
    assert_eq!(viewer_tag(&tags, "trel-other"), untagged());

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_resource_tags_carry_viewer_tag_uri() -> Result<()> {
    let path = format!("/v0/resource/{RESOURCE}/tags?viewer_id={VIEWER}");
    // `db mock` doesn't index resource tags: the first read fills the index
    get_request(&path).await?;
    let body = get_request(&path).await?;

    assert_eq!(
        viewer_tag(&body["tags"], "trel-shared"),
        tagged(VIEWER_RESOURCE_TAG_URI)
    );
    assert_eq!(viewer_tag(&body["tags"], "trel-other"), untagged());

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_tags_are_untagged_without_viewer() -> Result<()> {
    let post_tags = get_request(&format!("/v0/post/{AUTHOR}/{POST}/tags")).await?;
    let user_tags = get_request(&format!("/v0/user/{TAGGED_USER}/tags")).await?;
    let resource = get_request(&format!("/v0/resource/{RESOURCE}/tags")).await?;
    let post_view = get_request(&format!("/v0/post/{AUTHOR}/{POST}")).await?;
    let user_view = get_request(&format!("/v0/user/{TAGGED_USER}")).await?;
    let user_stream = post_request(
        "/v0/stream/users/by_ids",
        json!({ "user_ids": [TAGGED_USER], "viewer_id": null }),
    )
    .await?;

    for tags in [
        &post_tags,
        &user_tags,
        &resource["tags"],
        &post_view["tags"],
        &user_view["tags"],
        &user_stream[0]["tags"],
    ] {
        for label in ["trel-shared", "trel-other"] {
            assert_eq!(viewer_tag(tags, label), untagged(), "{label}");
        }
    }

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_views_and_streams_carry_viewer_tag_uri() -> Result<()> {
    let post_view = get_request(&format!("/v0/post/{AUTHOR}/{POST}?viewer_id={VIEWER}")).await?;
    let user_view = get_request(&format!("/v0/user/{TAGGED_USER}?viewer_id={VIEWER}")).await?;

    let post_stream = get_request(&format!(
        "/v0/stream/posts?source=author&author_id={AUTHOR}&viewer_id={VIEWER}"
    ))
    .await?;
    let stream_post = post_stream
        .as_array()
        .expect("post stream should be an array")
        .iter()
        .find(|p| p["details"]["id"] == POST)
        .expect("the author stream should hold the fixture post");

    let user_stream = post_request(
        "/v0/stream/users/by_ids",
        json!({ "user_ids": [TAGGED_USER], "viewer_id": VIEWER }),
    )
    .await?;
    let stream_user = &user_stream[0];
    assert_eq!(stream_user["details"]["id"], TAGGED_USER);

    // Two labels: the stream reads the graph, not a tag index `db mock` didn't fill
    let resource_stream = get_request(&format!(
        "/v0/stream/resources?tags=trel-shared,trel-other&viewer_id={VIEWER}"
    ))
    .await?;
    let stream_resource = resource_stream
        .as_array()
        .expect("resource stream should be an array")
        .iter()
        .find(|r| r["details"]["id"] == RESOURCE)
        .expect("the label stream should hold the fixture resource");

    let reads = [
        ("post view", &post_view["tags"], VIEWER_POST_TAG_URI),
        ("user view", &user_view["tags"], VIEWER_USER_TAG_URI),
        ("post stream", &stream_post["tags"], VIEWER_POST_TAG_URI),
        ("user stream", &stream_user["tags"], VIEWER_USER_TAG_URI),
        (
            "resource stream",
            &stream_resource["tags"],
            VIEWER_RESOURCE_TAG_URI,
        ),
    ];
    for (read, tags, viewer_uri) in reads {
        assert_eq!(
            viewer_tag(tags, "trel-shared"),
            tagged(viewer_uri),
            "{read}"
        );
        assert_eq!(viewer_tag(tags, "trel-other"), untagged(), "{read}");
    }

    Ok(())
}

/// The index lists the viewer as a tagger of a label the graph has no edge for.
#[tokio_shared_rt::test(shared)]
async fn test_tag_is_untagged_when_index_is_ahead_of_graph() -> Result<()> {
    // Ensure the server is running, for redis connection
    TestServiceServer::get_test_server().await;
    let mut redis_conn = get_redis_conn().await?;
    let sorted_set_key = format!("Sorted:Users:Tag:{DRIFT_USER}");
    let taggers_key = format!("User:Taggers:{DRIFT_USER}:trel-ahead");
    let _: () = redis_conn.zadd(&sorted_set_key, "trel-ahead", 1).await?;
    let _: () = redis_conn.sadd(&taggers_key, VIEWER).await?;

    let tags = get_request(&format!("/v0/user/{DRIFT_USER}/tags?viewer_id={VIEWER}")).await;

    let _: () = redis_conn.zrem(&sorted_set_key, "trel-ahead").await?;
    let _: () = redis_conn.del(&taggers_key).await?;

    let tags = tags?;
    assert_eq!(viewer_tag(&tags, "trel-ahead"), untagged());
    assert_eq!(
        viewer_tag(&tags, "trel-shared"),
        tagged(VIEWER_DRIFT_USER_TAG_URI)
    );

    Ok(())
}

// ##### Cache miss #####
// Each test deletes its own target's Redis tag keys. Only the resource streams, here and in
// `stream::resource`, also read one of them: see the nextest group `resource-tag-index`.

const MISS_AUTHOR: &str = "xxthkxjtb16wto9b7sud74dmsetqi54ewiisxaeadt6bhy6j9zmo";
const MISS_POST: &str = "2ZRT8G4VXQ0N0";
const MISS_USER: &str = "twtytiiy6girjjoep9amicagowbde5m7n3dcpjdeuehzstca13oo";
const MISS_RESOURCE: &str = "7a6e1c0de5f0a11ce0ffee0000000002";
const VIEWER_MISS_POST_TAG_URI: &str =
    "pubky://xcb38btb3ioaoxtinmwtmizfum1akeg5yajju67o6rrcxq47n76o/pub/mapky/tags/TRELMISSPV01";
const VIEWER_MISS_USER_TAG_URI: &str =
    "pubky://xcb38btb3ioaoxtinmwtmizfum1akeg5yajju67o6rrcxq47n76o/pub/mapky/tags/TRELMISSUV01";
const VIEWER_MISS_RESOURCE_TAG_URI: &str =
    "pubky://xcb38btb3ioaoxtinmwtmizfum1akeg5yajju67o6rrcxq47n76o/pub/mapky/tags/TRELMISSRV01";

/// Deletes a tag index: its sorted set and one taggers set per fixture label.
async fn clear_tag_index(sorted_set_key: &str, taggers_key_prefix: &str) -> Result<()> {
    // Ensure the server is running, for redis connection
    TestServiceServer::get_test_server().await;
    let mut redis_conn = get_redis_conn().await?;
    let _: () = redis_conn.del(sorted_set_key).await?;
    for label in ["trel-shared", "trel-other"] {
        let _: () = redis_conn
            .del(format!("{taggers_key_prefix}:{label}"))
            .await?;
    }
    Ok(())
}

/// Reads with and without the viewer, from a forced miss and then from the refilled
/// index: the viewer tagged `trel-shared` at their stored address only with the viewer
/// (so the fill cached nothing per viewer), and `trel-other` is always untagged.
async fn assert_viewer_tag_across_cache_miss(
    path: &str,
    tags_of: fn(&Value) -> &Value,
    viewer_uri: &str,
    sorted_set_key: &str,
    taggers_key_prefix: &str,
) -> Result<()> {
    let viewer_path = format!("{path}?viewer_id={VIEWER}");
    let reads = [
        ("anonymous miss", path, untagged()),
        ("viewer miss", viewer_path.as_str(), tagged(viewer_uri)),
        ("viewer hit", viewer_path.as_str(), tagged(viewer_uri)),
        ("anonymous hit", path, untagged()),
    ];
    for (read, read_path, expected_shared) in reads {
        if read.ends_with("miss") {
            clear_tag_index(sorted_set_key, taggers_key_prefix).await?;
        }
        let body = get_request(read_path).await?;
        assert_eq!(
            viewer_tag(tags_of(&body), "trel-shared"),
            expected_shared,
            "{read}"
        );
        assert_eq!(
            viewer_tag(tags_of(&body), "trel-other"),
            untagged(),
            "{read}"
        );
    }
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_tags_viewer_tag_on_cache_miss() -> Result<()> {
    assert_viewer_tag_across_cache_miss(
        &format!("/v0/post/{MISS_AUTHOR}/{MISS_POST}/tags"),
        |body| body,
        VIEWER_MISS_POST_TAG_URI,
        &format!("Sorted:Posts:Tag:{MISS_AUTHOR}:{MISS_POST}"),
        &format!("Post:Taggers:{MISS_AUTHOR}:{MISS_POST}"),
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_user_tags_viewer_tag_on_cache_miss() -> Result<()> {
    assert_viewer_tag_across_cache_miss(
        &format!("/v0/user/{MISS_USER}/tags"),
        |body| body,
        VIEWER_MISS_USER_TAG_URI,
        &format!("Sorted:Users:Tag:{MISS_USER}"),
        &format!("User:Taggers:{MISS_USER}"),
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_resource_tags_viewer_tag_on_cache_miss() -> Result<()> {
    assert_viewer_tag_across_cache_miss(
        &format!("/v0/resource/{MISS_RESOURCE}/tags"),
        |body| &body["tags"],
        VIEWER_MISS_RESOURCE_TAG_URI,
        &format!("Sorted:Resources:Tag:{MISS_RESOURCE}"),
        &format!("Resource:Taggers:{MISS_RESOURCE}"),
    )
    .await
}

// ##### Web of Trust #####
// The viewer follows a WoT tagger who tags the WoT post and user. The user test deletes the
// viewer's WoT cache keys for the WoT user. The WoT taggers test reads one of them too, but
// asserts only the viewer's tag, which comes from the global set and the graph whether or not
// the key exists.

const WOT_POST: &str = "2ZRT8G4VXQ0Q0";
const WOT_USER: &str = "ri3o8565ke5ngbknd9yg1g8gcxuuwk51owjwdioyqsxzqppy3dby";
const VIEWER_WOT_POST_TAG_URI: &str =
    "pubky://xcb38btb3ioaoxtinmwtmizfum1akeg5yajju67o6rrcxq47n76o/pub/mapky/tags/TRELWOTPV01";
const VIEWER_WOT_USER_TAG_URI: &str =
    "pubky://xcb38btb3ioaoxtinmwtmizfum1akeg5yajju67o6rrcxq47n76o/pub/mapky/tags/TRELWOTUV01";

#[tokio_shared_rt::test(shared)]
async fn test_wot_post_tags_carry_viewer_tag_uri() -> Result<()> {
    let tags = get_request(&format!(
        "/v0/post/{AUTHOR}/{WOT_POST}/tags?viewer_id={VIEWER}&depth=2"
    ))
    .await?;

    assert_eq!(
        viewer_tag(&tags, "trel-shared"),
        tagged(VIEWER_WOT_POST_TAG_URI)
    );
    assert_eq!(viewer_tag(&tags, "trel-other"), untagged());

    Ok(())
}

/// The first read misses the WoT cache and fills it, the next ones hit it.
#[tokio_shared_rt::test(shared)]
async fn test_wot_user_tags_viewer_tag_across_cache_miss() -> Result<()> {
    clear_tag_index(
        &format!("Cache:Sorted:Users:Tag:{VIEWER}:{WOT_USER}"),
        &format!("Cache:User:Taggers:{VIEWER}:{WOT_USER}"),
    )
    .await?;
    let tags_path = format!("/v0/user/{WOT_USER}/tags?viewer_id={VIEWER}&depth=2");
    let view_path = format!("/v0/user/{WOT_USER}?viewer_id={VIEWER}&depth=2");
    let tags_miss = get_request(&tags_path).await?;
    let tags_hit = get_request(&tags_path).await?;
    let view_hit = get_request(&view_path).await?;
    let reads = [
        ("tags miss", &tags_miss),
        ("tags hit", &tags_hit),
        ("view hit", &view_hit["tags"]),
    ];
    for (read, tags) in reads {
        assert_eq!(
            viewer_tag(tags, "trel-shared"),
            tagged(VIEWER_WOT_USER_TAG_URI),
            "{read}"
        );
        assert_eq!(viewer_tag(tags, "trel-other"), untagged(), "{read}");
    }

    Ok(())
}

/// The global taggers set decides whether the graph is read: on a global cache hit, and on a
/// WoT cache hit, whose sets leave the viewer out. The graph has the viewer's edge on all three
/// labels: one global set lists the viewer (the graph is read), one doesn't (no graph read, so
/// untagged) and one is missing (the graph decides; a global read drops the label). Global and
/// WoT tag details, and WoT taggers.
#[tokio_shared_rt::test(shared)]
async fn test_viewer_tag_follows_global_taggers_set() -> Result<()> {
    // Its global set lists the viewer, as `db mock` wrote it
    const MEMBER: &str = "trel-shared";
    const RULED_OUT: &str = "trel-wot-ruled-out";
    const UNINDEXED: &str = "trel-wot-unindexed";
    // Ensure the server is running, for redis connection
    TestServiceServer::get_test_server().await;
    let mut redis_conn = get_redis_conn().await?;
    let wot_sorted_set_key = format!("Cache:Sorted:Users:Tag:{VIEWER}:{DRIFT_USER}");
    let wot_taggers_key = |label: &str| format!("Cache:User:Taggers:{VIEWER}:{DRIFT_USER}:{label}");
    let global_taggers_key = |label: &str| format!("User:Taggers:{DRIFT_USER}:{label}");
    // The viewer has no WoT taggers on this user: fake a filled WoT cache
    for label in [MEMBER, RULED_OUT, UNINDEXED] {
        let _: () = redis_conn.zadd(&wot_sorted_set_key, label, 1).await?;
        let _: () = redis_conn.sadd(wot_taggers_key(label), AUTHOR).await?;
    }
    // Another tagger first, so the set stays when the viewer leaves it
    let _: () = redis_conn
        .sadd(global_taggers_key(RULED_OUT), AUTHOR)
        .await?;
    let _: () = redis_conn
        .srem(global_taggers_key(RULED_OUT), VIEWER)
        .await?;
    let _: () = redis_conn.del(global_taggers_key(UNINDEXED)).await?;

    let global_tags = get_request(&format!("/v0/user/{DRIFT_USER}/tags?viewer_id={VIEWER}")).await;
    let query = format!("viewer_id={VIEWER}&depth=2");
    let tags = get_request(&format!("/v0/user/{DRIFT_USER}/tags?{query}")).await;
    let taggers_path = |label: &str| format!("/v0/user/{DRIFT_USER}/taggers/{label}?{query}");
    let member_taggers = taggers_viewer_tag(&taggers_path(MEMBER)).await;
    let ruled_out_taggers = taggers_viewer_tag(&taggers_path(RULED_OUT)).await;
    let unindexed_taggers = taggers_viewer_tag(&taggers_path(UNINDEXED)).await;

    // Drop the faked WoT cache and restore the global sets `db mock` wrote
    let _: () = redis_conn.del(&wot_sorted_set_key).await?;
    let _: () = redis_conn.del(wot_taggers_key(MEMBER)).await?;
    for label in [RULED_OUT, UNINDEXED] {
        let _: () = redis_conn.del(wot_taggers_key(label)).await?;
        let _: () = redis_conn.del(global_taggers_key(label)).await?;
        let _: () = redis_conn.sadd(global_taggers_key(label), VIEWER).await?;
    }

    let global_tags = global_tags?;
    assert_eq!(
        viewer_tag(&global_tags, MEMBER),
        tagged(VIEWER_DRIFT_USER_TAG_URI)
    );
    assert_eq!(viewer_tag(&global_tags, RULED_OUT), untagged());
    let tags = tags?;
    assert_eq!(viewer_tag(&tags, MEMBER), tagged(VIEWER_DRIFT_USER_TAG_URI));
    assert_eq!(viewer_tag(&tags, RULED_OUT), untagged());
    assert_eq!(
        viewer_tag(&tags, UNINDEXED),
        tagged(VIEWER_DRIFT_UNINDEXED_TAG_URI)
    );
    assert_eq!(member_taggers?, tagged(VIEWER_DRIFT_USER_TAG_URI));
    assert_eq!(ruled_out_taggers?, untagged());
    assert_eq!(unindexed_taggers?, tagged(VIEWER_DRIFT_UNINDEXED_TAG_URI));

    Ok(())
}

// ##### Taggers #####

/// `(relationship, tag_uri)` of a taggers response.
async fn taggers_viewer_tag(path: &str) -> Result<(Value, Value)> {
    Ok(viewer_tag_fields(&get_request(path).await?))
}

/// Tagged with the viewer's stored address on `trel-shared`, untagged on `trel-other`,
/// and untagged on `trel-shared` without the viewer.
async fn assert_taggers_viewer_tag(
    taggers_path: impl Fn(&str) -> String,
    viewer_uri: &str,
) -> Result<()> {
    let with_viewer = |label: &str| format!("{}?viewer_id={VIEWER}", taggers_path(label));
    assert_eq!(
        taggers_viewer_tag(&with_viewer("trel-shared")).await?,
        tagged(viewer_uri)
    );
    assert_eq!(
        taggers_viewer_tag(&with_viewer("trel-other")).await?,
        untagged()
    );
    assert_eq!(
        taggers_viewer_tag(&taggers_path("trel-shared")).await?,
        untagged()
    );
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_taggers_carry_viewer_tag_uri() -> Result<()> {
    assert_taggers_viewer_tag(
        |label| format!("/v0/post/{AUTHOR}/{POST}/taggers/{label}"),
        VIEWER_POST_TAG_URI,
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_user_taggers_carry_viewer_tag_uri() -> Result<()> {
    assert_taggers_viewer_tag(
        |label| format!("/v0/user/{TAGGED_USER}/taggers/{label}"),
        VIEWER_USER_TAG_URI,
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_resource_taggers_carry_viewer_tag_uri() -> Result<()> {
    // `db mock` doesn't index resource tags: a tags read fills the taggers sets
    get_request(&format!("/v0/resource/{RESOURCE}/tags")).await?;
    assert_taggers_viewer_tag(
        |label| format!("/v0/resource/{RESOURCE}/tags/{label}/taggers"),
        VIEWER_RESOURCE_TAG_URI,
    )
    .await
}

/// The WoT taggers sets leave the viewer out, so the global taggers set flags the viewer,
/// whether or not the WoT cache is filled.
#[tokio_shared_rt::test(shared)]
async fn test_wot_user_taggers_carry_viewer_tag_uri() -> Result<()> {
    let path = format!("/v0/user/{WOT_USER}/taggers");
    let query = format!("viewer_id={VIEWER}&depth=2");

    let shared = taggers_viewer_tag(&format!("{path}/trel-shared?{query}")).await?;
    assert_eq!(shared, tagged(VIEWER_WOT_USER_TAG_URI));
    let other = taggers_viewer_tag(&format!("{path}/trel-other?{query}")).await?;
    assert_eq!(other, untagged());

    Ok(())
}

/// The taggers set lists the viewer on a label the graph has no edge for.
#[tokio_shared_rt::test(shared)]
async fn test_taggers_are_untagged_when_index_is_ahead_of_graph() -> Result<()> {
    // Ensure the server is running, for redis connection
    TestServiceServer::get_test_server().await;
    let mut redis_conn = get_redis_conn().await?;
    // Its own label: the tag details drift test adds and removes `trel-ahead`
    let taggers_key = format!("User:Taggers:{DRIFT_USER}:trel-taggers-ahead");
    let _: () = redis_conn.sadd(&taggers_key, VIEWER).await?;

    let body = get_request(&format!(
        "/v0/user/{DRIFT_USER}/taggers/trel-taggers-ahead?viewer_id={VIEWER}"
    ))
    .await;

    let _: () = redis_conn.del(&taggers_key).await?;

    let body = body?;
    assert_eq!(body["users"], serde_json::json!([VIEWER]));
    assert_eq!(viewer_tag_fields(&body), untagged());

    Ok(())
}
