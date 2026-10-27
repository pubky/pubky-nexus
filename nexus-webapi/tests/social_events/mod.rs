use crate::utils::{get_request, host_url, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;
use chrono::{Duration, Utc};
use nexus_common::{
    db::{exec_single_row, graph::Query},
    models::{
        post::{PostDetails, PostKind},
        social_event::{self, SocialContent},
    },
};
use serde_json::{json, Value};

/// A post id no other test process mints. `create_id` mints the clock's microsecond, strictly
/// increasing within one process but not across nextest's parallel processes, and `Post.id` is
/// unique in the graph, so two tests minting in the same microsecond collide. This draws a
/// random instant one to sixteen minutes back, once per process, counts up from it, and writes
/// it as a canonical 13-character timestamp id (Crockford base32, the dangling bit clear).
fn unique_post_id() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::BuildHasher;
    use std::sync::atomic::{AtomicI64, Ordering};
    use std::sync::OnceLock;

    const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    static NEXT: OnceLock<AtomicI64> = OnceLock::new();
    let next = NEXT.get_or_init(|| {
        let back = 60_000_000 + RandomState::new().hash_one(std::process::id()) % 900_000_000;
        AtomicI64::new(Utc::now().timestamp_micros() - back as i64)
    });
    let bits = (next.fetch_add(1, Ordering::Relaxed) as u128) << 1;
    (0..13)
        .map(|i| CROCKFORD[((bits >> (5 * (12 - i))) & 31) as usize] as char)
        .collect()
}

#[tokio_shared_rt::test(shared)]
async fn event_stream_includes_ongoing_events_and_excludes_touching_boundaries() -> Result<()> {
    host_url().await;
    let author = pubky::Keypair::random().public_key().to_z32();
    exec_single_row(
        Query::new("test_ongoing_author", "CREATE (:User {id:$author})")
            .param("author", author.clone()),
    )
    .await?;
    let now = chrono::DateTime::from_timestamp(Utc::now().timestamp(), 0).unwrap();
    let mut ids = Vec::new();
    for hours in [-1, 1, 3] {
        let id = unique_post_id();
        let content = json!({"title":"Overlapping event","description":"Time window regression",
            "start":(now+Duration::hours(hours)).format("%Y-%m-%dT%H:%M:%S").to_string(),
            "timeZone":"UTC","duration":"PT2H"})
        .to_string();
        PostDetails {
            author: author.clone(),
            id: id.clone(),
            uri: pubky_social_specs::post_uri_builder(author.clone(), id.clone()),
            content: content.clone(),
            kind: PostKind::Event,
            indexed_at: now.timestamp_millis(),
            ..Default::default()
        }
        .put_to_graph(None, None)
        .await?;
        social_event::sync(
            &author,
            &id,
            &content,
            None,
            &SocialContent::read(PostKind::Event, &content, now)?,
        )
        .await?;
        ids.push(id);
    }
    let default = get_request(&format!("/v1/stream/events?author={author}")).await?;
    // `/v0` is frozen: the event routes are `/v1` only.
    invalid_get_request(
        &format!("/v0/stream/events?author={author}"),
        StatusCode::NOT_FOUND,
    )
    .await?;
    assert_eq!(default.as_array().unwrap().len(), 3);
    assert_eq!(default[0]["post_id"], ids[0]);
    assert!(default[0]["start_at"].as_i64().unwrap() < now.timestamp_millis());
    let from = (now + Duration::hours(1)).timestamp_millis();
    let to = (now + Duration::hours(3)).timestamp_millis();
    let middle = get_request(&format!(
        "/v1/stream/events?author={author}&from={from}&to={to}"
    ))
    .await?;
    assert_eq!(middle.as_array().unwrap().len(), 1);
    assert_eq!(middle[0]["post_id"], ids[1]); // end == from and start == to are excluded
    let overlapping = get_request(&format!(
        "/v1/stream/events?author={author}&from={}&to={from}",
        now.timestamp_millis()
    ))
    .await?;
    assert_eq!(overlapping.as_array().unwrap().len(), 1);
    assert_eq!(overlapping[0]["post_id"], ids[0]);
    for id in ids {
        social_event::remove(&author, &id).await?;
    }
    exec_single_row(
        Query::new(
            "test_ongoing_cleanup",
            "MATCH (u:User {id:$author})-[:AUTHORED]->(p:Post) DETACH DELETE p,u",
        )
        .param("author", author),
    )
    .await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn event_routes_filters_rating_sorts_and_invalid_queries() -> Result<()> {
    host_url().await;
    let author = pubky::Keypair::random().public_key().to_z32();
    let viewer = pubky::Keypair::random().public_key().to_z32();
    exec_single_row(
        Query::new(
            "test_events_user",
            "CREATE (u:User {id:$author}), (v:User {id:$viewer}) CREATE (v)-[:FOLLOWS]->(u)",
        )
        .param("author", author.clone())
        .param("viewer", viewer.clone()),
    )
    .await?;
    let mut ids = Vec::new();
    for (day, rating) in [(1, 3.5), (2, 8.25)] {
        let id = unique_post_id();
        let uri = pubky_social_specs::post_uri_builder(author.clone(), id.clone());
        let content=json!({"title":"API calendar","description":"Native meeting description","start":(Utc::now()+Duration::days(day)).format("%Y-%m-%dT%H:%M:%S").to_string(),"timeZone":"UTC","duration":"PT2H","status":if day==1 {"confirmed"} else {"tentative"},"locations":[{"uri":format!("https://example.com/venue/{author}/{day}")}]}).to_string();
        let p = PostDetails {
            author: author.clone(),
            id: id.clone(),
            uri: uri.clone(),
            content: content.clone(),
            kind: PostKind::Event,
            indexed_at: Utc::now().timestamp_millis(),
            ..Default::default()
        };
        p.put_to_graph(None, None).await?;
        p.put_to_index(&author, None, false).await?;
        social_event::sync(
            &author,
            &id,
            &content,
            None,
            &SocialContent::read(PostKind::Event, &content, Utc::now())?,
        )
        .await?;
        exec_single_row(
            Query::new(
                "test_event_rating",
                "MATCH (p:Post {uri:$uri}) SET p.rating_avg=$rating,p.rating_count=2",
            )
            .param("uri", uri)
            .param("rating", rating),
        )
        .await?;
        ids.push(id);
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
    let body = get_request(&format!("/v1/stream/events?author={author}")).await?;
    assert_eq!(body.as_array().unwrap().len(), 2);
    assert_eq!(body[0]["post_id"], ids[0]);
    let location = format!("https://example.com/venue/{author}/2");
    let rid = nexus_common::universal_tag::normalize::resource_id(&location);
    for filter in [
        format!("locations={rid}"),
        format!("locations={location}"),
        "status=tentative".into(),
        format!("authors={author}"),
    ] {
        let filtered = get_request(&format!("/v1/stream/events?author={author}&{filter}")).await?;
        assert_eq!(
            filtered.as_array().unwrap().len(),
            if filter.starts_with("authors") { 2 } else { 1 }
        );
    }
    assert_eq!(
        get_request(&format!(
            "/v1/stream/events?author={author}&locations={rid}&status=confirmed"
        ))
        .await?,
        json!([])
    );
    let following = get_request(&format!(
        "/v1/stream/events?source=following&user_id={viewer}&author={author}"
    ))
    .await?;
    assert_eq!(following, body);
    let outsider = pubky::Keypair::random().public_key().to_z32();
    assert_eq!(
        get_request(&format!(
            "/v1/stream/events?source=following&user_id={outsider}&author={author}"
        ))
        .await?,
        json!([])
    );
    let from = body[0]["start_at"].as_i64().unwrap();
    let to = body[1]["start_at"].as_i64().unwrap();
    assert_eq!(
        get_request(&format!(
            "/v1/stream/events?author={author}&from={from}&to={to}"
        ))
        .await?
        .as_array()
        .unwrap()
        .len(),
        1
    );
    let detail = get_request(&format!("/v1/event/{author}/{}", ids[1])).await?;
    assert_eq!(detail["post"]["rating_avg"], 8.25);
    assert_eq!(detail["post"]["details"]["kind"], "event");
    assert_eq!(
        get_request(&format!(
            "/v1/event/{author}/{}/attendees?status=accepted",
            ids[1]
        ))
        .await?,
        json!([])
    );
    let posts = get_request(&format!(
        "/v1/stream/posts?source=author&author_id={author}&kind=event&sorting=rating"
    ))
    .await?;
    assert_eq!(posts[0]["details"]["id"], ids[1]);
    let search = get_request(&format!(
        "/v1/search/posts/by_content?q=calendar&kind=event&author={author}"
    ))
    .await?;
    assert_eq!(search.as_array().unwrap().len(), 2);
    let keys = get_request(&format!(
        "/v1/stream/posts/keys?source=author&author_id={author}&sorting=rating&limit=1"
    ))
    .await?;
    assert_eq!(keys["last_post_score"].as_f64(), Some(8.25));
    // `/v0` is frozen: an event post is a post of a kind it does not know, `unknown`, with no
    // rating; an event kind's name is read as `unknown` in `kind=` and refused in
    // `exclude_kinds=`, and `sorting=rating` is refused as the unknown sorting it is to `/v0`.
    let v0_post = get_request(&format!("/v0/post/{author}/{}", ids[1])).await?;
    assert_eq!(v0_post["details"]["kind"], "unknown", "{v0_post}");
    assert!(v0_post.get("rating_avg").is_none(), "{v0_post}");
    let v1_post = get_request(&format!("/v1/post/{author}/{}", ids[1])).await?;
    assert_eq!(v1_post["details"]["kind"], "event", "{v1_post}");
    assert_eq!(v1_post["rating_avg"], 8.25, "{v1_post}");
    let v0_events = get_request(&format!(
        "/v0/stream/posts?source=author&author_id={author}&kind=event"
    ))
    .await?;
    // `kind=event` is `unknown` to `/v0`, which matches every post it shows as `unknown`:
    // both event posts, each with that kind.
    let v0_listed: Vec<&Value> = v0_events
        .as_array()
        .unwrap()
        .iter()
        .map(|post| &post["details"]["id"])
        .collect();
    for id in &ids {
        assert!(
            v0_listed.contains(&&Value::from(id.as_str())),
            "{v0_events}"
        );
    }
    assert!(
        v0_events
            .as_array()
            .unwrap()
            .iter()
            .all(|post| post["details"]["kind"] == "unknown"),
        "{v0_events}"
    );
    let v0_all = get_request(&format!(
        "/v0/stream/posts?source=author&author_id={author}"
    ))
    .await?;
    assert!(!v0_all.as_array().unwrap().is_empty(), "{v0_all}");
    for post in v0_all.as_array().unwrap() {
        assert_eq!(post["details"]["kind"], "unknown", "{post}");
    }
    // Each refusal is the one `/v0` gave before these names existed, word for word: the query
    // extractor's rejection of a sorting variant it does not have, first even beside `app` or a
    // lone `user_id`, and the parsers' own words for an unknown kind or notification type.
    const POST_SORTINGS: &str = "Invalid input: Failed to deserialize query string: sorting: unknown variant `rating`, expected `timeline` or `total_engagement`";
    const RESOURCE_SORTINGS: &str = "Invalid input: Failed to deserialize query string: sorting: unknown variant `rating`, expected `timeline`";
    for (route, error) in [
        (
            format!("/v0/stream/posts?source=author&author_id={author}&sorting=rating"),
            POST_SORTINGS,
        ),
        (
            format!("/v0/stream/posts/keys?source=author&author_id={author}&sorting=rating"),
            POST_SORTINGS,
        ),
        (
            format!("/v0/search/posts/by_tag/native?sorting=rating&user_id={author}"),
            POST_SORTINGS,
        ),
        (
            "/v0/stream/resources?sorting=rating&app=rating.test".to_string(),
            RESOURCE_SORTINGS,
        ),
        (
            "/v0/stream/resources/ids?sorting=rating".to_string(),
            RESOURCE_SORTINGS,
        ),
    ] {
        let body = invalid_get_request(&route, StatusCode::BAD_REQUEST).await?;
        assert_eq!(body["error"], error, "{route}");
    }
    for (route, ending) in [
        (
            format!("/v0/stream/posts?source=author&author_id={author}&exclude_kinds=EVENT"),
            "Invalid content kind: event",
        ),
        (
            format!("/v0/user/{author}/notifications?types=event_changed"),
            "unknown notification type: event_changed",
        ),
    ] {
        let body = invalid_get_request(&route, StatusCode::BAD_REQUEST).await?;
        let error = body["error"].as_str().unwrap_or_default();
        assert!(error.ends_with(ending), "{route}: {error}");
    }
    let label = format!("native{}", &author[..8]);
    let collection = unique_post_id();
    let collection_uri = pubky_social_specs::post_uri_builder(author.clone(), collection.clone());
    exec_single_row(Query::new("test_event_filters","MATCH (u:User {id:$author}), (p:Post {id:$id}) CREATE (u)-[:TAGGED {label:$label,indexed_at:1}]->(p) CREATE (u)-[:AUTHORED]->(c:Post {id:$collection,uri:$uri,kind:'collection',content:$content,indexed_at:1,deleted:false})-[:COLLECTED]->(p)").param("author",author.clone()).param("id",ids[1].clone()).param("label",label.clone()).param("collection",collection.clone()).param("uri",collection_uri.clone()).param("content",json!({"name":"Public calendar","items":[{"uri":pubky_social_specs::post_uri_builder(author.clone(),ids[1].clone()),"note":"Featured"},{"uri":"https://example.com/external-calendar-item"}]}).to_string())).await?;
    assert_eq!(
        get_request(&format!("/v1/stream/events?author={author}&tags={label}"))
            .await?
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        get_request(&format!(
            "/v1/stream/events?author={author}&collection={collection_uri}"
        ))
        .await?[0]["post_id"],
        ids[1]
    );
    // Calendars are collections that contain events.
    let plain = unique_post_id();
    tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    let plain_collection = unique_post_id();
    exec_single_row(Query::new("test_plain_collection","MATCH (u:User {id:$author}) CREATE (u)-[:AUTHORED]->(n:Post {id:$plain,uri:$plain_uri,kind:'short',content:'plain',indexed_at:2}) CREATE (u)-[:AUTHORED]->(c:Post {id:$collection,uri:$uri,kind:'collection',content:'{}',indexed_at:3,deleted:false})-[:COLLECTED]->(n)")
        .param("author",author.clone()).param("plain",plain.clone()).param("plain_uri",pubky_social_specs::post_uri_builder(author.clone(), plain.clone()))
        .param("collection",plain_collection.clone()).param("uri",pubky_social_specs::post_uri_builder(author.clone(), plain_collection.clone()))).await?;
    let collections = |filter: &str| {
        format!("/v1/stream/posts/keys?source=author&author_id={author}&kind=collection{filter}")
    };
    assert_eq!(
        get_request(&collections("")).await?["post_keys"],
        json!([
            format!("{author}:{plain_collection}"),
            format!("{author}:{collection}")
        ])
    );
    assert_eq!(
        get_request(&collections("&item_kind=event")).await?["post_keys"],
        json!([format!("{author}:{collection}")])
    );
    assert_eq!(
        get_request(&collections("&item_kind=note")).await?["post_keys"],
        json!([format!("{author}:{plain_collection}")])
    );
    // `/v0` is frozen: it has no `item_kind`, so it reads none and lists every collection.
    assert_eq!(
        get_request(&collections("&item_kind=event").replacen("/v1/", "/v0/", 1)).await?
            ["post_keys"],
        json!([
            format!("{author}:{plain_collection}"),
            format!("{author}:{collection}")
        ])
    );
    let calendars = get_request(&format!(
        "/v1/stream/posts?source=author&author_id={author}&kind=collection&item_kind=event"
    ))
    .await?;
    assert!(calendars
        .as_array()
        .unwrap()
        .iter()
        .any(|post| post["details"]["id"] == collection));
    assert!(calendars
        .as_array()
        .unwrap()
        .iter()
        .all(|post| post["details"]["id"] != plain_collection));
    // collection= reads the collection's items, not every occurrence.
    assert_eq!(
        get_request(&format!("/v1/stream/events?collection={collection_uri}"))
            .await?
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        get_request(&format!(
            "/v1/stream/events?collection={}",
            pubky_social_specs::post_uri_builder(author.clone(), plain_collection.clone())
        ))
        .await?,
        json!([])
    );
    // Serve only newest per author/target, using the same projection as the watcher.
    for days in [30, 60] {
        let id = unique_post_id();
        let schedule = json!({"timeZone":"UTC","horizonDays":days,"for":location}).to_string();
        let post = PostDetails {
            author: author.clone(),
            id: id.clone(),
            uri: pubky_social_specs::post_uri_builder(author.clone(), id.clone()),
            content: schedule.clone(),
            kind: PostKind::Availability,
            indexed_at: Utc::now().timestamp_millis(),
            ..Default::default()
        };
        post.put_to_graph(None, None).await?;
        social_event::sync(
            &author,
            &id,
            &schedule,
            None,
            &SocialContent::read(PostKind::Availability, &schedule, Utc::now())?,
        )
        .await?;
    }
    let schedules = get_request(&format!("/v1/availability/{author}")).await?;
    assert_eq!(schedules.as_array().unwrap().len(), 1);
    assert_eq!(schedules[0]["schedule"]["horizonDays"], 60);
    assert_eq!(
        get_request(&format!("/v1/stream/availability?for={location}")).await?,
        schedules
    );
    assert_eq!(
        get_request(&format!(
            "/v1/stream/availability?author={viewer}&for={location}"
        ))
        .await?,
        json!([])
    );
    for endpoint in [
        "/v1/stream/events?status=unknown".to_string(),
        "/v1/stream/events?authors=invalid".into(),
        "/v1/stream/events?locations=relative".into(),
        format!(
            "/v1/stream/events?locations={}",
            vec![rid.as_str(); 101].join(",")
        ),
        "/v1/stream/posts?item_kind=event".into(),
        "/v1/stream/posts?kind=note&item_kind=event".into(),
        "/v1/stream/posts/keys?kind=collection&item_kind=unknown-kind".into(),
        format!("/v1/event/{author}/{}?limit_tags=101", ids[0]),
        format!("/v1/event/{author}/{}?limit_taggers=101", ids[0]),
        format!("/v1/event/{author}/{}?limit_attendees=201", ids[0]),
        "/v1/stream/availability".into(),
        "/v1/stream/availability?for=relative".into(),
        format!("/v1/availability/{author}?limit=0"),
    ] {
        invalid_get_request(&endpoint, StatusCode::BAD_REQUEST).await?;
    }
    let mut resource_ids = Vec::new();
    for rating in [3.5, 8.25] {
        let uri = format!("https://example.com/native-api/{author}/{rating}");
        let id = nexus_common::universal_tag::normalize::resource_id(&uri);
        resource_ids.push(id.clone());
        exec_single_row(Query::new("test_resource_ratings", "MATCH (u:User {id:$author}) CREATE (u)-[:TAGGED {label:$label,indexed_at:1}]->(r:Resource {id:$id,uri:$uri,scheme:'https',indexed_at:1,rating_avg:$rating,rating_count:1})")
          .param("author",author.clone()).param("label",label.clone()).param("id",id).param("uri",uri).param("rating",rating)).await?;
    }
    let resources =
        get_request(&format!("/v1/stream/resources?sorting=rating&tags={label}")).await?;
    assert_eq!(resources[0]["details"]["id"], resource_ids[1]);
    assert_eq!(resources[0]["rating_avg"], 8.25);
    let keys = get_request(&format!(
        "/v1/stream/resources/ids?sorting=rating&tags={label}&limit=1"
    ))
    .await?;
    assert_eq!(keys["last_score"].as_f64(), Some(8.25));
    for q in [
        "source=following",
        "source=unknown",
        "from=-1",
        "from=20&to=10",
        "from=0&to=9999999999999",
        "sort=venue",
        "limit=0",
        "limit=201",
        "skip=10001",
        "tags=",
        "collection=https://example.com/",
    ] {
        invalid_get_request(&format!("/v1/stream/events?{q}"), StatusCode::BAD_REQUEST).await?;
    }
    for q in ["status=going", "recurrence_id=bad"] {
        invalid_get_request(
            &format!("/v1/event/{author}/{}/attendees?{q}", ids[0]),
            StatusCode::BAD_REQUEST,
        )
        .await?;
    }
    invalid_get_request(
        &format!("/v1/event/{outsider}/{}", ids[0]),
        StatusCode::NOT_FOUND,
    )
    .await?;
    // All OpenAPI references resolve, including nested event and aggregate schemas.
    let api = get_request("/api-docs/v1/openapi.json").await?;
    fn refs(v: &serde_json::Value, root: &serde_json::Value) {
        if let Some(o) = v.as_object() {
            for (k, v) in o {
                if k == "$ref" {
                    assert!(
                        root.pointer(v.as_str().unwrap().trim_start_matches('#'))
                            .is_some(),
                        "{v}"
                    );
                } else {
                    refs(v, root);
                }
            }
        }
        if let Some(a) = v.as_array() {
            for v in a {
                refs(v, root);
            }
        }
    }
    refs(&api, &api);
    for id in &ids {
        social_event::remove(&author, id).await?;
    }
    exec_single_row(Query::new("test_events_cleanup","MATCH (u:User) WHERE u.id IN [$author,$viewer] OPTIONAL MATCH (u)-[:AUTHORED]->(p) DETACH DELETE p,u").param("author",author).param("viewer",viewer)).await?;
    Ok(())
}

/// `/v0` is frozen: it has no `event_changed` notification, so its list leaves one out and its
/// `types=` refuses the name; `/v1` lists it and filters by it.
#[tokio_shared_rt::test(shared)]
async fn event_changed_notifications_are_v1_only() -> Result<()> {
    use nexus_common::models::notification::{Notification, NotificationBody};
    host_url().await;
    let user = pubky::Keypair::random().public_key().to_z32();
    let follower = pubky::Keypair::random().public_key().to_z32();
    let event_uri = format!("pubky://{follower}/pub/social/v1/posts/0034A0X7NJ52G");
    for (timestamp, body) in [
        (
            1,
            NotificationBody::Follow {
                followed_by: follower.clone(),
            },
        ),
        (
            2,
            NotificationBody::EventChanged {
                event_uri: event_uri.clone(),
                recurrence_id: "2026-11-09T10:00:00".into(),
                post_uri: event_uri.clone(),
            },
        ),
    ] {
        Notification { timestamp, body }.store_for(&user).await?;
    }

    let v1 = get_request(&format!("/v1/user/{user}/notifications")).await?;
    let v1_types: Vec<&str> = v1
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["body"]["type"].as_str().unwrap())
        .collect();
    assert_eq!(v1_types, ["event_changed", "follow"], "{v1}");
    let v1_filtered = get_request(&format!(
        "/v1/user/{user}/notifications?types=event_changed"
    ))
    .await?;
    assert_eq!(v1_filtered.as_array().unwrap().len(), 1, "{v1_filtered}");

    let v0 = get_request(&format!("/v0/user/{user}/notifications")).await?;
    let v0_types: Vec<&str> = v0
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["body"]["type"].as_str().unwrap())
        .collect();
    assert_eq!(v0_types, ["follow"], "{v0}");
    // The hidden `event_changed` is the newest: a one-item `/v0` page is the follow beneath
    // it, not an empty page, and the page behind it is empty with no resume header.
    let client = httpc_test::new_client(host_url().await)?;
    let first = client
        .do_get(&format!("/v0/user/{user}/notifications?limit=1"))
        .await?;
    assert_eq!(first.status(), 200);
    let first_body = first.json_body()?;
    assert_eq!(first_body.as_array().map(Vec::len), Some(1), "{first_body}");
    assert_eq!(first_body[0]["body"]["type"], "follow", "{first_body}");
    assert_eq!(first.header("x-next-start"), None);
    let next = get_request(&format!("/v0/user/{user}/notifications?limit=1&skip=1")).await?;
    assert_eq!(next, json!([]), "{next}");
    invalid_get_request(
        &format!("/v0/user/{user}/notifications?types=follow,event_changed"),
        StatusCode::BAD_REQUEST,
    )
    .await?;
    // Twelve distinct names are `/v0`'s whole list; a thirteenth token is over its bound.
    let twelve = "follow,new_friend,lost_friend,tag_post,tag_profile,untag_post,untag_profile,reply,repost,mention,post_deleted,post_edited";
    get_request(&format!("/v0/user/{user}/notifications?types={twelve}")).await?;
    invalid_get_request(
        &format!("/v0/user/{user}/notifications?types={twelve},follow"),
        StatusCode::BAD_REQUEST,
    )
    .await?;
    Ok(())
}
