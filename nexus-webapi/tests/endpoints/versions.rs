//! The API answers under `/v0` and `/v1` from one route declaration, so the same request
//! gets the same answer on both, up to what `/v1` names differently: a post's kind. One
//! representative route per module, against the mock data, plus the kind names on every route
//! that carries one.

use crate::utils::{get_request, host_url, post_request};

use anyhow::Result;
use nexus_common::db::RedisOps;
use nexus_common::models::notification::{Notification, NotificationBody};
use nexus_common::models::post::PostKind;
use nexus_webapi::routes::v0::endpoints::{V0_PREFIX, V1_PREFIX};
use serde_json::{json, Value};

const PUBKY_USER: &str = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
const PUBKY_POST: &str = "2ZCW1TGR5BKG0";
const PUBKY_FILE: &str = "2ZK2H8P2T5NG0";
// Aldert, from docker/test-graph/mocks/posts.cypher
const AUTHOR: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
const AMSTERDAM: &str = "emq37ky6fbnaun7q1ris6rx3mqmw3a33so1txfesg9jj3ak9ryoy";
const NOTIFIED_USER: &str = "qrqnkbwqt8rd6aya9zruro76gyc4qs77qzek7xc11axbdtbu6esy";
const TAGGER: &str = "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y";
const TAG_ID: &str = "2Z1N8QBQK9EG0";
const RESOURCE_ID: &str = "450a72e3da164bfc3ac5f4056f9e5c7c"; // https://example.com/article

/// One route per module, relative to the version prefix. Every one of them is a 200 on the
/// mock data, so a difference in the answer is a difference between the mounts.
fn representative_routes() -> Vec<String> {
    let file_uri = url::form_urlencoded::byte_serialize(
        format!("pubky://{PUBKY_USER}/pub/pubky.app/files/{PUBKY_FILE}").as_bytes(),
    )
    .collect::<String>();
    vec![
        // info
        "/info".to_string(),
        // user
        format!("/user/{PUBKY_USER}/details"),
        // post
        format!("/post/{PUBKY_USER}/{PUBKY_POST}/details"),
        // stream
        format!("/stream/posts?author_id={AUTHOR}&source=author&sorting=timeline&limit=5"),
        // search
        "/search/users/by_name/Jo".to_string(),
        // tag
        format!("/tags/{TAGGER}/{TAG_ID}"),
        // resource
        format!("/resource/{RESOURCE_ID}/tags"),
        // notification
        format!("/user/{NOTIFIED_USER}/notifications?limit=2"),
        // bootstrap
        format!("/bootstrap/{AMSTERDAM}"),
        // events
        "/events?cursor=0&limit=5".to_string(),
        // file
        format!("/files/file/{file_uri}"),
    ]
}

/// A response's body as the client typed it: JSON as a value, anything else (the events
/// list is plain text) as its text.
fn body(response: &httpc_test::Response) -> Result<Value> {
    Ok(response
        .json_body()
        .or_else(|_| response.text_body().map(Value::String))?)
}

/// The bootstrap's `influencers` is a random window of the ranking on every call, the
/// `users` list carries their views, and its `posts` and `files` come back in no fixed
/// order, so two answers never match whole: compare the rest, with the posts in id order
/// and the files in uri order.
fn deterministic_part(route: &str, mut body: Value) -> Value {
    if route.starts_with("/bootstrap/") {
        body["ids"]["influencers"].take();
        body["users"].take();
        if let Some(posts) = body["posts"].as_array_mut() {
            posts.sort_by_key(|post| post["details"]["id"].to_string());
        }
        if let Some(files) = body["files"].as_array_mut() {
            files.sort_by_key(|file| file["uri"].to_string());
        }
    }
    body
}

/// `body` with every `kind` and `post_kind` under its stored name: the one thing `/v1` says
/// differently, so the rest of a `/v1` answer can be held against `/v0`'s.
fn with_stored_kind_names(body: Value) -> Value {
    match body {
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, value)| {
                    let value = match (key.as_str(), value.as_str()) {
                        ("kind" | "post_kind", Some(name)) => Value::from(stored_kind_name(name)),
                        _ => with_stored_kind_names(value),
                    };
                    (key, value)
                })
                .collect(),
        ),
        Value::Array(items) => {
            Value::Array(items.into_iter().map(with_stored_kind_names).collect())
        }
        other => other,
    }
}

/// The stored name of a kind named either way; a name neither version knows is kept, so
/// a stray value still fails the comparison.
fn stored_kind_name(name: &str) -> &str {
    match name {
        "note" => "short",
        "article" => "long",
        other => other,
    }
}

#[tokio_shared_rt::test(shared)]
async fn test_every_module_answers_the_same_on_both_versions() -> Result<()> {
    let client = httpc_test::new_client(host_url().await)?;

    for route in representative_routes() {
        let v0 = client.do_get(&format!("{V0_PREFIX}{route}")).await?;
        let v1 = client.do_get(&format!("{V1_PREFIX}{route}")).await?;
        assert_eq!(v0.status(), 200, "{V0_PREFIX}{route}");
        assert_eq!(v1.status(), v0.status(), "{route}");
        assert_eq!(
            with_stored_kind_names(deterministic_part(&route, body(&v1)?)),
            deterministic_part(&route, body(&v0)?),
            "{route}"
        );
    }

    Ok(())
}

// Bogota's short post and Eixample's long one, from docker/test-graph/mocks/posts.cypher.
const BOGOTA: &str = "ep441mndnsjeesenwz78r9paepm6e4kqm4ggiyy9uzpoe43eu9ny";
const SHORT_POST: &str = "00000039YD9BM";
const EIXAMPLE: &str = "8attbeo9ftu5nztqkcfw3gydksehr7jbspgfi64u4h8eo5e7dbiy";
const LONG_POST: &str = "4ZCW1TGL5BKG8";

/// The kind names each version gives a stored `short` and `long` post.
const KIND_NAMES: [(&str, &str, &str); 2] =
    [(V0_PREFIX, "short", "long"), (V1_PREFIX, "note", "article")];

/// A post view and its details name the kind the version's way, every other field the same.
#[tokio_shared_rt::test(shared)]
async fn test_post_view_and_details_name_the_kind_per_version() -> Result<()> {
    for (prefix, short, long) in KIND_NAMES {
        for (author, post, expected) in [(BOGOTA, SHORT_POST, short), (EIXAMPLE, LONG_POST, long)] {
            let view = get_request(&format!("{prefix}/post/{author}/{post}")).await?;
            assert_eq!(view["details"]["kind"], expected, "{prefix} view of {post}");
            assert_eq!(view["details"]["id"], post, "{prefix} view of {post}");
            assert!(view["details"]["deleted"].is_boolean(), "{prefix}: {view}");
            assert!(view["counts"].is_object(), "{prefix}: {view}");
            assert!(view["relationships"].is_object(), "{prefix}: {view}");

            let details = get_request(&format!("{prefix}/post/{author}/{post}/details")).await?;
            assert_eq!(details["kind"], expected, "{prefix} details of {post}");
            assert_eq!(details["id"], post, "{prefix} details of {post}");
            assert!(details["attachments"].is_array(), "{prefix}: {details}");
        }
    }
    Ok(())
}

/// A post stream and a `by_ids` stream name the kind the version's way on every post.
#[tokio_shared_rt::test(shared)]
async fn test_post_streams_name_the_kind_per_version() -> Result<()> {
    for (prefix, short, long) in KIND_NAMES {
        // The stored name is the filter on both versions until the v1 filters land.
        let stream = get_request(&format!(
            "{prefix}/stream/posts?kind=long&author_id={EIXAMPLE}&source=author"
        ))
        .await?;
        let posts = stream.as_array().expect("a post stream is an array");
        assert!(!posts.is_empty(), "{prefix}: {stream}");
        for post in posts {
            assert_eq!(post["details"]["kind"], long, "{prefix}: {post}");
        }

        let by_ids = post_request(
            &format!("{prefix}/stream/posts/by_ids"),
            json!({ "post_ids": [format!("{BOGOTA}:{SHORT_POST}"), format!("{EIXAMPLE}:{LONG_POST}")] }),
        )
        .await?;
        let posts = by_ids.as_array().expect("a post stream is an array");
        assert_eq!(posts.len(), 2, "{prefix}: {by_ids}");
        assert_eq!(posts[0]["details"]["id"], SHORT_POST, "{prefix}: {by_ids}");
        assert_eq!(posts[0]["details"]["kind"], short, "{prefix}: {by_ids}");
        assert_eq!(posts[1]["details"]["id"], LONG_POST, "{prefix}: {by_ids}");
        assert_eq!(posts[1]["details"]["kind"], long, "{prefix}: {by_ids}");
    }
    Ok(())
}

/// A notification list names `post_kind` the version's way, `unknown` included, and changes
/// nothing else in a body.
///
/// The recipient is an id the notification tests only ever use as a follower, so its list
/// starts empty; the seed is idempotent, a sorted set holds one copy of a member.
#[tokio_shared_rt::test(shared)]
async fn test_notifications_name_the_post_kind_per_version() -> Result<()> {
    const RECIPIENT: &str = "xeeo9uagnox3obkwzud91bkjecgd5eaumbix4b13k8i7yorzjony";
    const REPLIER: &str = "17775cing48kciy3enntgjwhw7yptxz6zw8k8czcddzk1yruqhcy";
    crate::utils::server::TestServiceServer::get_test_server().await;

    let reply = NotificationBody::Reply {
        replied_by: REPLIER.to_string(),
        parent_post_uri: format!("pubky://{RECIPIENT}/pub/social/v1/posts/0000000000002"),
        reply_uri: format!("pubky://{REPLIER}/pub/social/v1/posts/0000000000003"),
        post_kind: PostKind::Short,
    };
    let mention = NotificationBody::Mention {
        mentioned_by: REPLIER.to_string(),
        post_uri: format!("pubky://{REPLIER}/pub/social/v1/posts/0000000000004"),
        post_kind: PostKind::Long,
    };
    // A row stored before `post_kind` existed reads as an unknown kind.
    let legacy = format!(
        r#"{{"type":"tag_post","tagged_by":"{REPLIER}","tag_label":"legacy","post_uri":"pubky://{RECIPIENT}/pub/pubky.app/posts/0000000000001"}}"#
    );
    Notification::put_index_sorted_set(
        &["Notification", RECIPIENT],
        &[
            (3000.0, serde_json::to_string(&reply)?.as_str()),
            (2000.0, serde_json::to_string(&mention)?.as_str()),
            (1000.0, legacy.as_str()),
        ],
        None,
        None,
    )
    .await
    .map_err(|e| anyhow::anyhow!("{e}"))?;

    let v0 = get_request(&format!("{V0_PREFIX}/user/{RECIPIENT}/notifications")).await?;
    let v1 = get_request(&format!("{V1_PREFIX}/user/{RECIPIENT}/notifications")).await?;
    let (v0_items, v1_items) = (v0.as_array().unwrap(), v1.as_array().unwrap());
    assert_eq!(v0_items.len(), 3, "{v0}");
    assert_eq!(v1_items.len(), 3, "{v1}");

    assert_eq!(v0_items[0]["body"]["post_kind"], "short");
    assert_eq!(v1_items[0]["body"]["post_kind"], "note");
    assert_eq!(v0_items[1]["body"]["post_kind"], "long");
    assert_eq!(v1_items[1]["body"]["post_kind"], "article");
    assert_eq!(v0_items[2]["body"]["post_kind"], "unknown");
    assert_eq!(v1_items[2]["body"]["post_kind"], "unknown");
    assert_eq!(with_stored_kind_names(v1), v0);

    Ok(())
}
