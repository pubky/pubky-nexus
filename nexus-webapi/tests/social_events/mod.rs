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
use pubky_social_specs::traits::TimestampId;
use serde_json::json;

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
        let id = pubky_social_specs::PubkySocialPost::new(
            "".into(),
            pubky_social_specs::PubkySocialPostKind::Note,
            None,
            None,
            vec![],
        )
        .create_id();
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
    let default = get_request(&format!("/v0/stream/events?author={author}")).await?;
    assert_eq!(default.as_array().unwrap().len(), 3);
    assert_eq!(default[0]["post_id"], ids[0]);
    assert!(default[0]["start_at"].as_i64().unwrap() < now.timestamp_millis());
    let from = (now + Duration::hours(1)).timestamp_millis();
    let to = (now + Duration::hours(3)).timestamp_millis();
    let middle = get_request(&format!(
        "/v0/stream/events?author={author}&from={from}&to={to}"
    ))
    .await?;
    assert_eq!(middle.as_array().unwrap().len(), 1);
    assert_eq!(middle[0]["post_id"], ids[1]); // end == from and start == to are excluded
    let overlapping = get_request(&format!(
        "/v0/stream/events?author={author}&from={}&to={from}",
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
        let id = pubky_social_specs::PubkySocialPost::new(
            "".into(),
            pubky_social_specs::PubkySocialPostKind::Note,
            None,
            None,
            vec![],
        )
        .create_id();
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
    let body = get_request(&format!("/v0/stream/events?author={author}")).await?;
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
        let filtered = get_request(&format!("/v0/stream/events?author={author}&{filter}")).await?;
        assert_eq!(
            filtered.as_array().unwrap().len(),
            if filter.starts_with("authors") { 2 } else { 1 }
        );
    }
    assert_eq!(
        get_request(&format!(
            "/v0/stream/events?author={author}&locations={rid}&status=confirmed"
        ))
        .await?,
        json!([])
    );
    let following = get_request(&format!(
        "/v0/stream/events?source=following&user_id={viewer}&author={author}"
    ))
    .await?;
    assert_eq!(following, body);
    let outsider = pubky::Keypair::random().public_key().to_z32();
    assert_eq!(
        get_request(&format!(
            "/v0/stream/events?source=following&user_id={outsider}&author={author}"
        ))
        .await?,
        json!([])
    );
    let from = body[0]["start_at"].as_i64().unwrap();
    let to = body[1]["start_at"].as_i64().unwrap();
    assert_eq!(
        get_request(&format!(
            "/v0/stream/events?author={author}&from={from}&to={to}"
        ))
        .await?
        .as_array()
        .unwrap()
        .len(),
        1
    );
    let detail = get_request(&format!("/v0/event/{author}/{}", ids[1])).await?;
    assert_eq!(detail["post"]["rating_avg"], 8.25);
    assert_eq!(detail["post"]["details"]["kind"], "event");
    assert_eq!(
        get_request(&format!(
            "/v0/event/{author}/{}/attendees?status=accepted",
            ids[1]
        ))
        .await?,
        json!([])
    );
    let posts = get_request(&format!(
        "/v0/stream/posts?source=author&author_id={author}&kind=event&sorting=rating"
    ))
    .await?;
    assert_eq!(posts[0]["details"]["id"], ids[1]);
    let search = get_request(&format!(
        "/v0/search/posts/by_content?q=calendar&kind=event&author={author}"
    ))
    .await?;
    assert_eq!(search.as_array().unwrap().len(), 2);
    let keys = get_request(&format!(
        "/v0/stream/posts/keys?source=author&author_id={author}&sorting=rating&limit=1"
    ))
    .await?;
    assert_eq!(keys["last_post_score"].as_f64(), Some(8.25));
    let label = format!("native{}", &author[..8]);
    let collection = pubky_social_specs::PubkySocialPost::new(
        "".into(),
        pubky_social_specs::PubkySocialPostKind::Note,
        None,
        None,
        vec![],
    )
    .create_id();
    let collection_uri = pubky_social_specs::post_uri_builder(author.clone(), collection.clone());
    exec_single_row(Query::new("test_event_filters","MATCH (u:User {id:$author}), (p:Post {id:$id}) CREATE (u)-[:TAGGED {label:$label,indexed_at:1}]->(p) CREATE (u)-[:AUTHORED]->(c:Post {id:$collection,uri:$uri,kind:'collection',content:$content,indexed_at:1,deleted:false})-[:COLLECTED]->(p)").param("author",author.clone()).param("id",ids[1].clone()).param("label",label.clone()).param("collection",collection.clone()).param("uri",collection_uri.clone()).param("content",json!({"name":"Public calendar","items":[{"uri":pubky_social_specs::post_uri_builder(author.clone(),ids[1].clone()),"note":"Featured"},{"uri":"https://example.com/external-calendar-item"}]}).to_string())).await?;
    assert_eq!(
        get_request(&format!("/v0/stream/events?author={author}&tags={label}"))
            .await?
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        get_request(&format!(
            "/v0/stream/events?author={author}&collection={collection_uri}"
        ))
        .await?[0]["post_id"],
        ids[1]
    );
    // Calendars are collections that contain events.
    let plain = pubky_social_specs::PubkySocialPost::new(
        "plain".into(),
        pubky_social_specs::PubkySocialPostKind::Note,
        None,
        None,
        vec![],
    )
    .create_id();
    tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    let plain_collection = pubky_social_specs::PubkySocialPost::new(
        "plain collection".into(),
        pubky_social_specs::PubkySocialPostKind::Note,
        None,
        None,
        vec![],
    )
    .create_id();
    exec_single_row(Query::new("test_plain_collection","MATCH (u:User {id:$author}) CREATE (u)-[:AUTHORED]->(n:Post {id:$plain,uri:$plain_uri,kind:'short',content:'plain',indexed_at:2}) CREATE (u)-[:AUTHORED]->(c:Post {id:$collection,uri:$uri,kind:'collection',content:'{}',indexed_at:3,deleted:false})-[:COLLECTED]->(n)")
        .param("author",author.clone()).param("plain",plain.clone()).param("plain_uri",pubky_social_specs::post_uri_builder(author.clone(), plain.clone()))
        .param("collection",plain_collection.clone()).param("uri",pubky_social_specs::post_uri_builder(author.clone(), plain_collection.clone()))).await?;
    let collections = |filter: &str| {
        format!("/v0/stream/posts/keys?source=author&author_id={author}&kind=collection{filter}")
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
        get_request(&collections("&item_kind=short")).await?["post_keys"],
        json!([format!("{author}:{plain_collection}")])
    );
    let calendars = get_request(&format!(
        "/v0/stream/posts?source=author&author_id={author}&kind=collection&item_kind=event"
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
        get_request(&format!("/v0/stream/events?collection={collection_uri}"))
            .await?
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        get_request(&format!(
            "/v0/stream/events?collection={}",
            pubky_social_specs::post_uri_builder(author.clone(), plain_collection.clone())
        ))
        .await?,
        json!([])
    );
    // Serve only newest per author/target, using the same projection as the watcher.
    for days in [30, 60] {
        let id = pubky_social_specs::PubkySocialPost::new(
            "".into(),
            pubky_social_specs::PubkySocialPostKind::Note,
            None,
            None,
            vec![],
        )
        .create_id();
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
    let schedules = get_request(&format!("/v0/availability/{author}")).await?;
    assert_eq!(schedules.as_array().unwrap().len(), 1);
    assert_eq!(schedules[0]["schedule"]["horizonDays"], 60);
    assert_eq!(
        get_request(&format!("/v0/stream/availability?for={location}")).await?,
        schedules
    );
    assert_eq!(
        get_request(&format!(
            "/v0/stream/availability?author={viewer}&for={location}"
        ))
        .await?,
        json!([])
    );
    for endpoint in [
        "/v0/stream/events?status=unknown".to_string(),
        "/v0/stream/events?authors=invalid".into(),
        "/v0/stream/events?locations=relative".into(),
        format!(
            "/v0/stream/events?locations={}",
            vec![rid.as_str(); 101].join(",")
        ),
        "/v0/stream/posts?item_kind=event".into(),
        "/v0/stream/posts?kind=short&item_kind=event".into(),
        "/v0/stream/posts/keys?kind=collection&item_kind=unknown-kind".into(),
        format!("/v0/event/{author}/{}?limit_tags=101", ids[0]),
        format!("/v0/event/{author}/{}?limit_taggers=101", ids[0]),
        format!("/v0/event/{author}/{}?limit_attendees=201", ids[0]),
        "/v0/stream/availability".into(),
        "/v0/stream/availability?for=relative".into(),
        format!("/v0/availability/{author}?limit=0"),
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
        get_request(&format!("/v0/stream/resources?sorting=rating&tags={label}")).await?;
    assert_eq!(resources[0]["details"]["id"], resource_ids[1]);
    assert_eq!(resources[0]["rating_avg"], 8.25);
    let keys = get_request(&format!(
        "/v0/stream/resources/ids?sorting=rating&tags={label}&limit=1"
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
        invalid_get_request(&format!("/v0/stream/events?{q}"), StatusCode::BAD_REQUEST).await?;
    }
    for q in ["status=going", "recurrence_id=bad"] {
        invalid_get_request(
            &format!("/v0/event/{author}/{}/attendees?{q}", ids[0]),
            StatusCode::BAD_REQUEST,
        )
        .await?;
    }
    invalid_get_request(
        &format!("/v0/event/{outsider}/{}", ids[0]),
        StatusCode::NOT_FOUND,
    )
    .await?;
    // All OpenAPI references resolve, including nested event and aggregate schemas.
    let api = get_request("/api-docs/v0/openapi.json").await?;
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
