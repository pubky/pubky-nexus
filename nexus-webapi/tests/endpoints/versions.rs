//! The API answers under `/v0` and `/v1` from one route declaration, so the same request
//! gets the same answer on both, up to what `/v1` names differently, a post's kind, in the
//! responses and in the `kind` and `exclude_kinds` filters, and what `/v1` leaves out, the
//! bookmarks and the file details. One representative route per module, against the mock
//! data, plus the kind names on every route that carries one, the bookmarks on every route
//! that carries them and the file details on every route that carries them.

use crate::utils::{get_request, host_url, invalid_get_request, post_request};

use anyhow::Result;
use axum::http::StatusCode;
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

/// The `/files/file/{file_id}` path of the mock file, its URI form-encoded.
fn file_details_route() -> String {
    let file_uri = url::form_urlencoded::byte_serialize(
        format!("pubky://{PUBKY_USER}/pub/pubky.app/files/{PUBKY_FILE}").as_bytes(),
    )
    .collect::<String>();
    format!("/files/file/{file_uri}")
}

/// One route per module both versions serve, relative to the version prefix. Every one of
/// them is a 200 on the mock data, so a difference in the answer is a difference between the
/// mounts. The file module is `/v0`'s alone: `test_file_routes_are_v0_only`.
fn representative_routes() -> Vec<String> {
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
/// `users` list carries their views, and its `posts` come back in no fixed order, so two
/// answers never match whole: compare the rest, with the posts in id order.
fn deterministic_part(route: &str, mut body: Value) -> Value {
    if route.starts_with("/bootstrap/") {
        body["ids"]["influencers"].take();
        body["users"].take();
        if let Some(posts) = body["posts"].as_array_mut() {
            posts.sort_by_key(|post| post["details"]["id"].to_string());
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

/// `body` without the `bookmark` of a post view, the `bookmarks` of a user's counts and the
/// `files` of the bootstrap: the fields `/v1` has no counterpart for, taken out of `/v0`'s
/// answer so the rest is compared. `/v1` never writes them, so on that side this changes
/// nothing, which `test_bootstrap_has_no_bookmarks_on_v1`, `test_bootstrap_has_no_files_on_v1`
/// and their siblings assert on their own.
fn without_v0_only_fields(body: Value) -> Value {
    match body {
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .filter(|(key, _)| key != "bookmark" && key != "bookmarks" && key != "files")
                .map(|(key, value)| (key, without_v0_only_fields(value)))
                .collect(),
        ),
        Value::Array(items) => {
            Value::Array(items.into_iter().map(without_v0_only_fields).collect())
        }
        other => other,
    }
}

/// A `/v1` answer as `/v0` would give it: the stored kind names, and no bookmark or file
/// fields on either side.
fn comparable(v1: Value, v0: Value) -> (Value, Value) {
    (
        without_v0_only_fields(with_stored_kind_names(v1)),
        without_v0_only_fields(v0),
    )
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
        let (v1, v0) = comparable(
            deterministic_part(&route, body(&v1)?),
            deterministic_part(&route, body(&v0)?),
        );
        assert_eq!(v1, v0, "{route}");
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
        let stream = get_request(&format!(
            "{prefix}/stream/posts?kind={long}&author_id={EIXAMPLE}&source=author"
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

/// The post ids of a stream, in order.
fn ids_of(stream: &Value) -> Vec<String> {
    stream
        .as_array()
        .expect("a post stream is an array")
        .iter()
        .map(|post| post["details"]["id"].as_str().unwrap().to_string())
        .collect()
}

/// The `post_key`s of a content search, sorted.
fn post_keys_of(results: &Value) -> Vec<String> {
    let mut keys: Vec<String> = results
        .as_array()
        .expect("search results are an array")
        .iter()
        .map(|result| result["post_key"].as_str().unwrap().to_string())
        .collect();
    keys.sort();
    keys
}

/// The error a 400 carries.
async fn rejection(route: &str) -> Result<String> {
    let body = invalid_get_request(route, StatusCode::BAD_REQUEST).await?;
    Ok(body["error"]
        .as_str()
        .unwrap_or_else(|| panic!("{route}: no error in {body}"))
        .to_string())
}

/// `kind` and `exclude_kinds` take each version's names on the post stream, the post keys and
/// the content search, and select the same posts: the keys line up with the stream, and the
/// search under `kind=article` on `/v1` finds what `kind=long` finds on `/v0`.
#[tokio_shared_rt::test(shared)]
async fn test_kind_filters_take_each_versions_names() -> Result<()> {
    let mut found_per_version = Vec::new();
    for (prefix, short, long) in KIND_NAMES {
        let by_author = format!("source=author&author_id={EIXAMPLE}&limit=50");

        let stream = get_request(&format!("{prefix}/stream/posts?{by_author}&kind={long}")).await?;
        let ids = ids_of(&stream);
        assert!(!ids.is_empty(), "{prefix}: {stream}");
        for post in stream.as_array().unwrap() {
            assert_eq!(post["details"]["kind"], long, "{prefix}: {post}");
        }
        let keys = get_request(&format!(
            "{prefix}/stream/posts/keys?{by_author}&kind={long}"
        ))
        .await?;
        let keys: Vec<String> = keys["post_keys"]
            .as_array()
            .unwrap_or_else(|| panic!("{prefix}: {keys}"))
            .iter()
            .map(|key| key.as_str().unwrap().to_string())
            .collect();
        let expected: Vec<String> = ids.iter().map(|id| format!("{EIXAMPLE}:{id}")).collect();
        assert_eq!(keys, expected, "{prefix}");

        let excluded = get_request(&format!(
            "{prefix}/stream/posts?{by_author}&exclude_kinds={short},{long}"
        ))
        .await?;
        let posts = excluded.as_array().unwrap();
        assert!(!posts.is_empty(), "{prefix}: {excluded}");
        for post in posts {
            let kind = post["details"]["kind"].as_str().unwrap();
            assert!(kind != short && kind != long, "{prefix}: {post}");
        }

        let found = get_request(&format!(
            "{prefix}/search/posts/by_content?q=post&kind={long}&limit=100"
        ))
        .await?;
        let found = post_keys_of(&found);
        assert!(!found.is_empty(), "{prefix}: no `{long}` post holds `post`");
        found_per_version.push(found);
    }
    assert_eq!(found_per_version[0], found_per_version[1]);
    Ok(())
}

/// On `/v1` a stored name, an unknown token and a differently cased name are a 400 naming the
/// parameter, on every route with a kind filter; `/v0` keeps its lenient `kind` and its strict
/// `exclude_kinds` as they were.
#[tokio_shared_rt::test(shared)]
async fn test_v1_kind_filters_reject_other_names() -> Result<()> {
    for (query, parameter) in [
        ("kind=short", "`kind`"),
        ("kind=foo", "`kind`"),
        ("kind=Note", "`kind`"),
        ("exclude_kinds=short", "`exclude_kinds`"),
        ("exclude_kinds=note,long", "`exclude_kinds`"),
    ] {
        for route in ["/stream/posts", "/stream/posts/keys"] {
            let error = rejection(&format!("{V1_PREFIX}{route}?{query}")).await?;
            assert!(error.contains(parameter), "{route}?{query}: {error}");
            assert!(error.contains("note, article"), "{route}?{query}: {error}");
        }
    }
    for query in ["kind=short", "kind=foo", "kind=Note"] {
        let error = rejection(&format!(
            "{V1_PREFIX}/search/posts/by_content?q=post&{query}"
        ))
        .await?;
        assert!(error.contains("`kind`"), "{query}: {error}");
    }

    // `/v0` reads any `kind` name, a `/v1` one included, and matches nothing with it.
    let v0 = get_request(&format!(
        "{V0_PREFIX}/stream/posts?source=author&author_id={EIXAMPLE}&kind=note"
    ))
    .await?;
    assert_eq!(v0, json!([]));
    let v0 = get_request(&format!(
        "{V0_PREFIX}/search/posts/by_content?q=post&kind=article"
    ))
    .await?;
    assert_eq!(v0, json!([]));
    // `/v0`'s `exclude_kinds` is strict against the stored names, in any case.
    rejection(&format!("{V0_PREFIX}/stream/posts?exclude_kinds=note")).await?;
    let v0 = get_request(&format!(
        "{V0_PREFIX}/stream/posts?source=author&author_id={EIXAMPLE}&exclude_kinds=Long"
    ))
    .await?;
    assert!(!v0.as_array().unwrap().is_empty(), "{v0}");
    Ok(())
}

/// `unknown` is a kind a `/v1` client may filter by or exclude, so it can leave out the posts
/// it cannot render; `/v0`'s `exclude_kinds` keeps rejecting it.
#[tokio_shared_rt::test(shared)]
async fn test_unknown_is_a_filter_name_on_v1() -> Result<()> {
    let by_author = format!("source=author&author_id={EIXAMPLE}&limit=50");
    let all = ids_of(&get_request(&format!("{V1_PREFIX}/stream/posts?{by_author}")).await?);
    assert!(!all.is_empty());

    // The mock data holds no post of an unrecognised kind, so the filter selects none and
    // the exclusion leaves every post in.
    for route in ["/stream/posts", "/stream/posts/keys"] {
        let only_unknown =
            get_request(&format!("{V1_PREFIX}{route}?{by_author}&kind=unknown")).await?;
        let without_unknown = get_request(&format!(
            "{V1_PREFIX}{route}?{by_author}&exclude_kinds=unknown"
        ))
        .await?;
        if route == "/stream/posts" {
            assert_eq!(only_unknown, json!([]), "{route}");
            assert_eq!(ids_of(&without_unknown), all, "{route}");
        } else {
            assert_eq!(only_unknown["post_keys"], json!([]), "{route}");
            assert_eq!(
                without_unknown["post_keys"].as_array().map(Vec::len),
                Some(all.len()),
                "{route}"
            );
        }
    }
    let found = get_request(&format!(
        "{V1_PREFIX}/search/posts/by_content?q=post&kind=unknown"
    ))
    .await?;
    assert_eq!(found, json!([]));

    rejection(&format!("{V0_PREFIX}/stream/posts?exclude_kinds=unknown")).await?;
    rejection(&format!(
        "{V0_PREFIX}/stream/posts/keys?exclude_kinds=unknown"
    ))
    .await?;
    Ok(())
}

/// The rejections around the kind filters hold on `/v1` as on `/v0`: `kind` with
/// `exclude_kinds`, and either with a source that cannot take them.
#[tokio_shared_rt::test(shared)]
async fn test_kind_filter_rejections_hold_on_both_versions() -> Result<()> {
    for (prefix, short, long) in KIND_NAMES {
        let anchored = format!("author_id={BOGOTA}&post_id={SHORT_POST}");
        for query in [
            format!("kind={short}&exclude_kinds={long}"),
            format!("source=collection&{anchored}&kind={short}"),
            format!("source=post_collections&{anchored}&exclude_kinds={long}"),
            format!("source=post_replies&{anchored}&kind={short}"),
            format!("source=author_replies&author_id={BOGOTA}&exclude_kinds={short}"),
        ] {
            for route in ["/stream/posts", "/stream/posts/keys"] {
                let error = rejection(&format!("{prefix}{route}?{query}")).await?;
                assert!(error.contains("kind"), "{prefix}{route}?{query}: {error}");
            }
        }
    }
    Ok(())
}

// The post `PUBKY_USER` bookmarked, from docker/test-graph/mocks, and the user with the most
// bookmarks and the one whose counts the user suite pins.
const BOOKMARKED_POST: &str = "2ZCW1TGR5BKG0";
const BOOKMARK_ID: &str = "2Z9PFGC3WWWW0";
const BOOKMARKER: &str = "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo";
const COUNTED_USER: &str = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";

/// `/v0` serves the bookmark route; `/v1` has no such route, so it is a 404 there like any
/// path the API does not know.
#[tokio_shared_rt::test(shared)]
async fn test_bookmark_route_is_v0_only() -> Result<()> {
    let client = httpc_test::new_client(host_url().await)?;
    let route = format!("/post/{PUBKY_USER}/{BOOKMARKED_POST}/bookmark?viewer_id={PUBKY_USER}");

    let v0 = client.do_get(&format!("{V0_PREFIX}{route}")).await?;
    assert_eq!(v0.status(), 200, "{V0_PREFIX}{route}");
    let bookmark = v0.json_body()?;
    assert_eq!(bookmark["id"], BOOKMARK_ID, "{bookmark}");
    assert!(bookmark["indexed_at"].is_number(), "{bookmark}");

    let v1 = client.do_get(&format!("{V1_PREFIX}{route}")).await?;
    assert_eq!(v1.status(), 404, "{V1_PREFIX}{route}");
    // Without the viewer too: the route itself is gone, not one answer of it.
    let v1 = client
        .do_get(&format!(
            "{V1_PREFIX}/post/{PUBKY_USER}/{BOOKMARKED_POST}/bookmark"
        ))
        .await?;
    assert_eq!(v1.status(), 404);
    Ok(())
}

/// A `/v0` post view carries the viewer's `bookmark`; the `/v1` view of the same post for the
/// same viewer has no `bookmark` key and is otherwise the same.
#[tokio_shared_rt::test(shared)]
async fn test_post_view_has_no_bookmark_on_v1() -> Result<()> {
    let route = format!("/post/{PUBKY_USER}/{BOOKMARKED_POST}?viewer_id={PUBKY_USER}");
    let mut v0 = get_request(&format!("{V0_PREFIX}{route}")).await?;
    let v1 = get_request(&format!("{V1_PREFIX}{route}")).await?;

    assert_eq!(v0["bookmark"]["id"], BOOKMARK_ID, "{v0}");
    assert!(v1.get("bookmark").is_none(), "{v1}");
    for field in ["details", "counts", "tags", "relationships"] {
        assert!(v1.get(field).is_some(), "{field}: {v1}");
    }
    v0.as_object_mut().unwrap().remove("bookmark");
    assert_eq!(with_stored_kind_names(v1), v0);

    // A viewer who did not bookmark the post: `/v0` says `null`, `/v1` says nothing.
    let route = format!("/post/{PUBKY_USER}/{BOOKMARKED_POST}?viewer_id={AUTHOR}");
    let v0 = get_request(&format!("{V0_PREFIX}{route}")).await?;
    let v1 = get_request(&format!("{V1_PREFIX}{route}")).await?;
    assert_eq!(v0["bookmark"], Value::Null, "{v0}");
    assert!(v1.get("bookmark").is_none(), "{v1}");

    // The streams serve the same view: no `bookmark` on `/v1` there either.
    let stream = get_request(&format!(
        "{V1_PREFIX}/stream/posts?source=author&author_id={PUBKY_USER}&viewer_id={PUBKY_USER}&limit=50"
    ))
    .await?;
    let posts = stream.as_array().expect("a post stream is an array");
    assert!(!posts.is_empty(), "{stream}");
    for post in posts {
        assert!(post.get("bookmark").is_none(), "{post}");
    }
    Ok(())
}

/// `/v0` counts a user's bookmarks; the `/v1` counts of the same user have no `bookmarks` key
/// and are otherwise the same.
#[tokio_shared_rt::test(shared)]
async fn test_user_counts_have_no_bookmarks_on_v1() -> Result<()> {
    let route = format!("/user/{COUNTED_USER}/counts");
    let mut v0 = get_request(&format!("{V0_PREFIX}{route}")).await?;
    let v1 = get_request(&format!("{V1_PREFIX}{route}")).await?;

    assert_eq!(v0["bookmarks"], 14, "{v0}");
    assert!(v1.get("bookmarks").is_none(), "{v1}");
    for count in [
        "tagged",
        "tags",
        "unique_tags",
        "posts",
        "replies",
        "collections",
        "following",
        "followers",
        "friends",
    ] {
        assert!(v1[count].is_number(), "{count}: {v1}");
    }
    v0.as_object_mut().unwrap().remove("bookmarks");
    assert_eq!(v1, v0);

    // An unknown user is a 404 on both.
    let unknown = "qca6wzjg4okp6g1hwr9g8hmx1po1jpoirjfau9ejsws1qz3t7iiy";
    for prefix in [V0_PREFIX, V1_PREFIX] {
        invalid_get_request(
            &format!("{prefix}/user/{unknown}/counts"),
            StatusCode::NOT_FOUND,
        )
        .await?;
    }
    Ok(())
}

/// Every key of `value` named `name`, at any depth.
fn keys_named(value: &Value, name: &str) -> usize {
    match value {
        Value::Object(fields) => fields
            .iter()
            .map(|(key, value)| usize::from(key == name) + keys_named(value, name))
            .sum(),
        Value::Array(items) => items.iter().map(|item| keys_named(item, name)).sum(),
        _ => 0,
    }
}

/// A `/v0` user view carries `counts.bookmarks`; the `/v1` view of the same user has no
/// `bookmarks` key under `counts`, or anywhere, and is otherwise the same.
#[tokio_shared_rt::test(shared)]
async fn test_user_view_has_no_bookmarks_count_on_v1() -> Result<()> {
    let route = format!("/user/{COUNTED_USER}?viewer_id={BOOKMARKER}");
    let mut v0 = get_request(&format!("{V0_PREFIX}{route}")).await?;
    let v1 = get_request(&format!("{V1_PREFIX}{route}")).await?;

    assert_eq!(v0["counts"]["bookmarks"], 14, "{v0}");
    assert!(v1["counts"].get("bookmarks").is_none(), "{v1}");
    assert_eq!(keys_named(&v1, "bookmarks"), 0, "{v1}");
    for field in [
        "details",
        "counts",
        "tags",
        "relationship",
        "social_graph_status",
    ] {
        assert!(v1.get(field).is_some(), "{field}: {v1}");
    }
    for count in [
        "tagged",
        "tags",
        "posts",
        "following",
        "followers",
        "friends",
    ] {
        assert!(v1["counts"][count].is_number(), "{count}: {v1}");
    }
    v0["counts"].as_object_mut().unwrap().remove("bookmarks");
    assert_eq!(v1, v0);

    // An unknown user is a 404 on both.
    let unknown = "qca6wzjg4okp6g1hwr9g8hmx1po1jpoirjfau9ejsws1qz3t7iiy";
    for prefix in [V0_PREFIX, V1_PREFIX] {
        invalid_get_request(&format!("{prefix}/user/{unknown}"), StatusCode::NOT_FOUND).await?;
    }
    Ok(())
}

/// The user streams serve the version's view: on `/v0` every user's counts carry
/// `bookmarks`, on `/v1` none do, and the pages are otherwise the same. The `by_ids` list,
/// the username search and a `source=` page, the three routes that return user views.
#[tokio_shared_rt::test(shared)]
async fn test_user_streams_have_no_bookmarks_count_on_v1() -> Result<()> {
    let by_ids = json!({ "user_ids": [COUNTED_USER, BOOKMARKER], "viewer_id": PUBKY_USER });
    let v0 = post_request(&format!("{V0_PREFIX}/stream/users/by_ids"), by_ids.clone()).await?;
    let v1 = post_request(&format!("{V1_PREFIX}/stream/users/by_ids"), by_ids).await?;
    let users = v0.as_array().expect("a user stream is an array");
    assert_eq!(users.len(), 2, "{v0}");
    assert_eq!(users[0]["counts"]["bookmarks"], 14, "{v0}");
    assert!(users[1]["counts"]["bookmarks"].is_number(), "{v0}");
    assert_eq!(v1.as_array().map(Vec::len), Some(2), "{v1}");
    assert_eq!(keys_named(&v1, "bookmarks"), 0, "{v1}");
    assert_eq!(v1, without_v0_only_fields(v0));

    let pages = [
        "/stream/users/username?username=Jo".to_string(),
        format!("/stream/users?source=followers&user_id={COUNTED_USER}&limit=5"),
    ];
    for route in pages {
        let v0 = get_request(&format!("{V0_PREFIX}{route}")).await?;
        let v1 = get_request(&format!("{V1_PREFIX}{route}")).await?;
        let users = v0.as_array().expect("a user stream is an array");
        assert!(!users.is_empty(), "{V0_PREFIX}{route}: {v0}");
        for user in users {
            assert!(user["counts"]["bookmarks"].is_number(), "{route}: {user}");
        }
        let v1_users = v1.as_array().expect("a user stream is an array");
        assert_eq!(v1_users.len(), users.len(), "{route}: {v1}");
        for user in v1_users {
            assert!(user["counts"].is_object(), "{route}: {user}");
            assert!(user["counts"].get("bookmarks").is_none(), "{route}: {user}");
        }
        assert_eq!(keys_named(&v1, "bookmarks"), 0, "{route}: {v1}");
        // The username search lists one order; a `source=` page may not.
        if route.contains("username") {
            assert_eq!(v1, without_v0_only_fields(v0), "{route}");
        }
    }
    Ok(())
}

/// The bootstrap embeds users and posts in the version's shape: on `/v0` every user's counts
/// carry `bookmarks` and every post its `bookmark`; on `/v1` no key named either is anywhere
/// in the payload, and the posts name their kind the `/v1` way.
#[tokio_shared_rt::test(shared)]
async fn test_bootstrap_has_no_bookmarks_on_v1() -> Result<()> {
    let route = format!("/bootstrap/{AMSTERDAM}");
    let v0 = get_request(&format!("{V0_PREFIX}{route}")).await?;
    let v1 = get_request(&format!("{V1_PREFIX}{route}")).await?;

    let users = v0["users"].as_array().expect("users");
    let posts = v0["posts"].as_array().expect("posts");
    assert!(!users.is_empty() && !posts.is_empty(), "{v0}");
    for user in users {
        assert!(user["counts"]["bookmarks"].is_number(), "{user}");
    }
    for post in posts {
        assert!(post.get("bookmark").is_some(), "{post}");
    }

    let users = v1["users"].as_array().expect("users");
    let posts = v1["posts"].as_array().expect("posts");
    assert!(!users.is_empty() && !posts.is_empty(), "{v1}");
    for user in users {
        assert!(user["counts"].is_object(), "{user}");
        assert!(user["counts"].get("bookmarks").is_none(), "{user}");
    }
    for post in posts {
        assert!(post.get("bookmark").is_none(), "{post}");
        let kind = post["details"]["kind"].as_str().expect("a kind");
        assert!(kind != "short" && kind != "long", "{post}");
    }
    assert_eq!(keys_named(&v1, "bookmarks"), 0, "{v1}");
    assert_eq!(keys_named(&v1, "bookmark"), 0, "{v1}");
    // The rest of the payload is there as on `/v0`.
    for field in ["ids", "indexed", "notifications"] {
        assert!(v1.get(field).is_some(), "{field}: {v1}");
    }
    Ok(())
}

/// `source=bookmarks` is a stream on `/v0` and an unknown source on `/v1`, a 400 that lists the
/// sources a `/v1` client may use; every other source answers on both.
#[tokio_shared_rt::test(shared)]
async fn test_bookmarks_source_is_unknown_on_v1() -> Result<()> {
    for route in ["/stream/posts", "/stream/posts/keys"] {
        let query = format!("{route}?source=bookmarks&observer_id={BOOKMARKER}");
        let v0 = get_request(&format!("{V0_PREFIX}{query}")).await?;
        let listed = if route == "/stream/posts" {
            v0.as_array().map(Vec::len)
        } else {
            v0["post_keys"].as_array().map(Vec::len)
        };
        assert!(listed.is_some_and(|n| n > 0), "{V0_PREFIX}{query}: {v0}");

        let error = rejection(&format!("{V1_PREFIX}{query}")).await?;
        assert!(error.contains("`bookmarks`"), "{route}: {error}");
        assert!(error.contains("`following`"), "{route}: {error}");
        assert!(error.contains("`all`"), "{route}: {error}");
        // The observer alone is no reason to reject: the source is.
        let error = rejection(&format!("{V1_PREFIX}{route}?source=bookmarks")).await?;
        assert!(error.contains("`bookmarks`"), "{route}: {error}");

        // Every other source still answers the same on both.
        for source in [
            format!("source=following&observer_id={BOOKMARKER}"),
            format!("source=author&author_id={AUTHOR}"),
            "source=all".to_string(),
        ] {
            let v0 = get_request(&format!("{V0_PREFIX}{route}?{source}&limit=5")).await?;
            let v1 = get_request(&format!("{V1_PREFIX}{route}?{source}&limit=5")).await?;
            let (v1, v0) = comparable(v1, v0);
            assert_eq!(v1, v0, "{route}?{source}");
        }
    }
    Ok(())
}

// Cairo's post with two attachments whose `File` nodes the mock graph holds, from
// docker/test-graph/mocks/posts.cypher and files.cypher.
const CAIRO: &str = "f5tcy5gtgzshipr6pag6cn9uski3s8tjare7wd3n7enmyokgjk1o";
const POST_WITH_ATTACHMENTS: &str = "00000039YD9DP";
const ATTACHMENT_FILE_IDS: [&str; 2] = ["2ZK3A1B2C3D40", "2ZK3E5F6G7H80"];
// A second mock file, for the `by_ids` list.
const OTHER_FILE_OWNER: &str = "sfgetccnq7s3h57a7imf6n7k5fqxus33yg85f1ndhnrnofjdmhjy";
const OTHER_FILE: &str = "2ZK1VCJN4YE00";

/// `/v0` serves the two file details routes; `/v1` has no such routes, so they are a 404
/// there like any path the API does not know, with or without a body a `/v0` client would
/// send.
#[tokio_shared_rt::test(shared)]
async fn test_file_routes_are_v0_only() -> Result<()> {
    let client = httpc_test::new_client(host_url().await)?;

    let details = file_details_route();
    let v0 = client.do_get(&format!("{V0_PREFIX}{details}")).await?;
    assert_eq!(v0.status(), 200, "{V0_PREFIX}{details}");
    let file = v0.json_body()?;
    assert_eq!(file["id"], PUBKY_FILE, "{file}");
    assert_eq!(file["owner_id"], PUBKY_USER, "{file}");
    let v1 = client.do_get(&format!("{V1_PREFIX}{details}")).await?;
    assert_eq!(v1.status(), 404, "{V1_PREFIX}{details}");

    let uris = json!({ "uris": [
        format!("pubky://{PUBKY_USER}/pub/pubky.app/files/{PUBKY_FILE}"),
        format!("pubky://{OTHER_FILE_OWNER}/pub/pubky.app/files/{OTHER_FILE}"),
    ] });
    let v0 = client
        .do_post(&format!("{V0_PREFIX}/files/by_ids"), uris.clone())
        .await?;
    assert_eq!(v0.status(), 200, "{V0_PREFIX}/files/by_ids");
    let files = v0.json_body()?;
    assert_eq!(files.as_array().map(Vec::len), Some(2), "{files}");
    let v1 = client
        .do_post(&format!("{V1_PREFIX}/files/by_ids"), uris)
        .await?;
    assert_eq!(v1.status(), 404, "{V1_PREFIX}/files/by_ids");
    // Without a body too: the route itself is gone, not one answer of it.
    let v1 = client
        .do_post(&format!("{V1_PREFIX}/files/by_ids"), Value::Null)
        .await?;
    assert_eq!(v1.status(), 404);
    Ok(())
}

/// A `/v0` post view, stream and `by_ids` stream carry `attachments_metadata` when asked with
/// `include_attachment_metadata`; the `/v1` ones never do, asked or not, and the flag is read
/// as any unknown parameter is: a value `/v0` rejects with 400 leaves `/v1` at 200, and the
/// answer is the same as without it.
#[tokio_shared_rt::test(shared)]
async fn test_post_views_have_no_attachments_metadata_on_v1() -> Result<()> {
    let view = format!("/post/{CAIRO}/{POST_WITH_ATTACHMENTS}");
    let flagged = format!("{view}?include_attachment_metadata=true");

    let v0 = get_request(&format!("{V0_PREFIX}{flagged}")).await?;
    let ids: Vec<&str> = v0["attachments_metadata"]
        .as_array()
        .unwrap_or_else(|| panic!("{v0}"))
        .iter()
        .map(|file| file["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ATTACHMENT_FILE_IDS, "{v0}");
    let v0_plain = get_request(&format!("{V0_PREFIX}{view}")).await?;
    assert!(v0_plain.get("attachments_metadata").is_none(), "{v0_plain}");

    let v1_flagged = get_request(&format!("{V1_PREFIX}{flagged}")).await?;
    let v1_plain = get_request(&format!("{V1_PREFIX}{view}")).await?;
    for body in [&v1_flagged, &v1_plain] {
        assert!(body.get("attachments_metadata").is_none(), "{body}");
        assert_eq!(keys_named(body, "attachments_metadata"), 0, "{body}");
        assert_eq!(
            body["details"]["attachments"].as_array().map(Vec::len),
            Some(2),
            "{body}"
        );
        for field in ["details", "counts", "tags", "relationships"] {
            assert!(body.get(field).is_some(), "{field}: {body}");
        }
    }
    assert_eq!(v1_flagged, v1_plain);
    assert_eq!(
        with_stored_kind_names(v1_plain.clone()),
        without_v0_only_fields(v0_plain)
    );

    // A value that is not a boolean: `/v0` reads a flag and rejects it, `/v1` reads nothing.
    let junk = format!("{view}?include_attachment_metadata=maybe");
    invalid_get_request(&format!("{V0_PREFIX}{junk}"), StatusCode::BAD_REQUEST).await?;
    let v1_junk = get_request(&format!("{V1_PREFIX}{junk}")).await?;
    assert_eq!(v1_junk, v1_plain);

    // The streams serve the same view.
    let stream = format!("/stream/posts?author_id={CAIRO}&source=author&sorting=timeline&limit=20&include_attachment_metadata=true");
    let v0 = get_request(&format!("{V0_PREFIX}{stream}")).await?;
    let v0_post = v0
        .as_array()
        .expect("a post stream is an array")
        .iter()
        .find(|post| post["details"]["id"] == POST_WITH_ATTACHMENTS)
        .unwrap_or_else(|| panic!("{v0}"));
    assert_eq!(
        v0_post["attachments_metadata"].as_array().map(Vec::len),
        Some(2),
        "{v0_post}"
    );
    let v1 = get_request(&format!("{V1_PREFIX}{stream}")).await?;
    let posts = v1.as_array().expect("a post stream is an array");
    assert!(
        posts
            .iter()
            .any(|post| post["details"]["id"] == POST_WITH_ATTACHMENTS),
        "{v1}"
    );
    assert_eq!(keys_named(&v1, "attachments_metadata"), 0, "{v1}");
    let v1_unflagged = get_request(&format!(
        "{V1_PREFIX}/stream/posts?author_id={CAIRO}&source=author&sorting=timeline&limit=20"
    ))
    .await?;
    assert_eq!(v1, v1_unflagged);

    let by_ids = json!({
        "post_ids": [format!("{CAIRO}:{POST_WITH_ATTACHMENTS}")],
        "include_attachment_metadata": true
    });
    let v0 = post_request(&format!("{V0_PREFIX}/stream/posts/by_ids"), by_ids.clone()).await?;
    assert_eq!(
        v0[0]["attachments_metadata"].as_array().map(Vec::len),
        Some(2),
        "{v0}"
    );
    let v1 = post_request(&format!("{V1_PREFIX}/stream/posts/by_ids"), by_ids).await?;
    assert_eq!(v1.as_array().map(Vec::len), Some(1), "{v1}");
    assert_eq!(v1[0]["details"]["id"], POST_WITH_ATTACHMENTS, "{v1}");
    assert_eq!(keys_named(&v1, "attachments_metadata"), 0, "{v1}");
    Ok(())
}

/// A `/v0` bootstrap carries `files`, the `File` nodes of the posts' attachments; the `/v1`
/// bootstrap of the same user has no `files` key, or any key so named, and the rest of the
/// payload as on `/v0`.
#[tokio_shared_rt::test(shared)]
async fn test_bootstrap_has_no_files_on_v1() -> Result<()> {
    // The user whose bootstrap the user suite pins: its stream holds a post whose two
    // attachments are Cairo's files, so `/v0` lists both.
    let user = "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy";
    let route = format!("/bootstrap/{user}");
    let v0 = get_request(&format!("{V0_PREFIX}{route}")).await?;
    let v1 = get_request(&format!("{V1_PREFIX}{route}")).await?;

    let mut files: Vec<&str> = v0["files"]
        .as_array()
        .unwrap_or_else(|| panic!("{v0}"))
        .iter()
        .map(|file| file["id"].as_str().unwrap())
        .collect();
    files.sort_unstable();
    assert_eq!(files, ATTACHMENT_FILE_IDS, "{v0}");

    assert!(v1.get("files").is_none(), "{v1}");
    assert_eq!(keys_named(&v1, "files"), 0, "{v1}");
    assert_eq!(keys_named(&v1, "attachments_metadata"), 0, "{v1}");
    for field in ["users", "posts", "ids", "indexed", "notifications"] {
        assert!(v1.get(field).is_some(), "{field}: {v1}");
    }
    assert_eq!(v1["indexed"], true, "{v1}");
    assert_eq!(
        v1["ids"]["stream"].as_array().map(Vec::len),
        v0["ids"]["stream"].as_array().map(Vec::len),
        "{v1}"
    );
    Ok(())
}
