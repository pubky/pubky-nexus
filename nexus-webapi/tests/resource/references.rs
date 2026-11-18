use crate::utils::{get_request, invalid_get_request, post_request};
use anyhow::Result;
use axum::http::StatusCode;
use nexus_common::db::{get_neo4j_graph, graph::Query};
use nexus_common::models::event::resource_changes::ResourceChangeKind;
use nexus_common::models::post::{PostDetails, PostStream};
use nexus_common::models::resource::references;
use nexus_common::universal_tag::normalize::resource_id;
use serde_json::{json, Value};

/// A resource's replies are newest first, and replies of one time are ordered by author and
/// then by post id: pages of one walk them all, none skipped or repeated.
#[tokio_shared_rt::test(shared)]
async fn resource_reference_routes_order_ties_by_author_then_post() -> Result<()> {
    crate::utils::server::TestServiceServer::get_test_server().await;
    let mut authors = [(); 2].map(|_| pubky::Keypair::random().public_key().to_z32());
    authors.sort();
    let [first, second] = &authors;
    let target = format!("https://example.com/api-reference-ties/{first}");
    let rid = resource_id(&target);
    let graph = get_neo4j_graph()?;
    for author in &authors {
        graph
            .run(
                Query::new("reference_ties_user", "CREATE (:User {id: $author})")
                    .param("author", author.clone()),
            )
            .await?;
    }
    let stamp = chrono::Utc::now().timestamp_micros();
    let post_id = |n: i64| format!("{:013X}", stamp + 2 * n);
    // Written in an order unlike the one they are listed in
    let written = [
        (second, post_id(0), 2000),
        (first, post_id(2), 2000),
        (first, post_id(1), 2000),
        (second, post_id(3), 3000),
    ];
    for (author, id, indexed_at) in &written {
        let uri = format!("pubky://{author}/pub/social/v1/posts/{id}");
        let post = PostDetails {
            author: author.to_string(),
            id: id.clone(),
            uri: uri.clone(),
            content: "tied reply".into(),
            indexed_at: *indexed_at,
            kind: nexus_common::models::post::PostKind::Short,
            attachments: vec![],
            lock: None,
            deleted: false,
        };
        post.put_to_graph(None, None).await?;
        references::sync(
            &post,
            vec![references::reference(
                author,
                &format!("{uri}/{id}.json"),
                &target,
                ResourceChangeKind::Reply,
            )?],
        )
        .await?;
    }
    let listed = [
        (second.clone(), post_id(3)),
        (first.clone(), post_id(1)),
        (first.clone(), post_id(2)),
        (second.clone(), post_id(0)),
    ];
    let keys = |page: &Value| -> Vec<(String, String)> {
        page.as_array()
            .unwrap()
            .iter()
            .map(|post| {
                (
                    post["details"]["author"].as_str().unwrap().to_string(),
                    post["details"]["id"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    };
    let mut paged = Vec::new();
    for skip in 0..listed.len() {
        let page = get_request(&format!("/v1/resource/{rid}/replies?skip={skip}&limit=1")).await?;
        paged.extend(keys(&page));
    }
    assert_eq!(paged, listed);
    let all = get_request(&format!("/v1/resource/{rid}/replies")).await?;
    assert_eq!(keys(&all), listed);
    assert_eq!(
        get_request(&format!("/v1/resource/{rid}/replies?skip={}", listed.len())).await?,
        json!([])
    );

    for (author, id, _) in &written {
        references::retract(author, id).await?;
    }
    for author in &authors {
        graph.run(Query::new("reference_ties_cleanup", "MATCH (u:User {id:$author}) OPTIONAL MATCH (u)-[:AUTHORED]->(p:Post) DETACH DELETE p, u").param("author", author.clone())).await?;
    }
    graph
        .run(
            Query::new(
                "reference_ties_resource_cleanup",
                "MATCH (r:Resource {id: $rid}) DETACH DELETE r",
            )
            .param("rid", rid),
        )
        .await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn resource_reference_routes_counts_pagination_and_validation() -> Result<()> {
    crate::utils::server::TestServiceServer::get_test_server().await;
    let author = pubky::Keypair::random().public_key().to_z32();
    let target = format!("https://example.com/api-references/{author}");
    let rid = resource_id(&target);
    let graph = get_neo4j_graph()?;
    graph
        .run(
            Query::new("reference_api_user", "CREATE (:User {id: $author})")
                .param("author", author.clone()),
        )
        .await?;
    let mut ids = vec![];
    for (offset, kind) in [
        ResourceChangeKind::Reply,
        ResourceChangeKind::Reply,
        ResourceChangeKind::Embed,
    ]
    .into_iter()
    .enumerate()
    {
        let id = format!(
            "{:013X}",
            chrono::Utc::now().timestamp_micros() + offset as i64
        );
        let uri = format!("pubky://{author}/pub/social/v1/posts/{id}");
        let post = PostDetails {
            author: author.clone(),
            id: id.clone(),
            uri: uri.clone(),
            content: "resource view".into(),
            indexed_at: 1000 + offset as i64,
            kind: nexus_common::models::post::PostKind::Short,
            attachments: vec![],
            lock: None,
            deleted: false,
        };
        post.put_to_graph(None, None).await?;
        references::sync(
            &post,
            vec![references::reference(
                &author,
                &format!("{uri}/{id}.json"),
                &target,
                kind,
            )?],
        )
        .await?;
        ids.push(id);
    }
    let view = get_request(&format!("/v1/resource/{rid}/tags")).await?;
    assert_eq!(
        (
            view["replies_count"].as_u64(),
            view["embeds_count"].as_u64(),
            view["collections_count"].as_u64()
        ),
        (Some(2), Some(1), Some(0))
    );
    // The counts are on every `/v1` resource view, the batch's and the stream's too, and on
    // no `/v0` one.
    const COUNTS: [&str; 3] = ["replies_count", "embeds_count", "collections_count"];
    let counts_of = |view: &Value| -> Vec<Option<u64>> {
        COUNTS.iter().map(|count| view[*count].as_u64()).collect()
    };
    let batch = post_request(
        "/v1/stream/resources/by_ids",
        json!({"resource_ids": [rid]}),
    )
    .await?;
    assert_eq!(counts_of(&batch[0]), [Some(2), Some(1), Some(0)], "{batch}");
    let label = format!("refs{}", &author[..8]);
    graph
        .run(
            Query::new(
                "reference_api_tag",
                "MATCH (u:User {id: $author}), (r:Resource {id: $rid})
                 CREATE (u)-[:TAGGED {label: $label, id: 'REFAPITAG', indexed_at: 1000}]->(r)",
            )
            .param("author", author.clone())
            .param("rid", rid.clone())
            .param("label", label.clone()),
        )
        .await?;
    let in_stream = |stream: &Value| -> Value {
        stream
            .as_array()
            .unwrap()
            .iter()
            .find(|view| view["details"]["id"] == rid.as_str())
            .unwrap_or_else(|| panic!("{rid} missing from {stream}"))
            .clone()
    };
    let v1_stream = get_request(&format!("/v1/stream/resources?tags={label}")).await?;
    assert_eq!(
        counts_of(&in_stream(&v1_stream)),
        [Some(2), Some(1), Some(0)],
        "{v1_stream}"
    );
    let v0_stream = get_request(&format!("/v0/stream/resources?tags={label}")).await?;
    let v0_item = in_stream(&v0_stream);
    for count in COUNTS {
        assert!(v0_item.get(count).is_none(), "{v0_item}");
    }
    // `/v0` is frozen: its envelope has no count and it serves no reference route.
    let v0_view = get_request(&format!("/v0/resource/{rid}/tags")).await?;
    for count in ["replies_count", "embeds_count", "collections_count"] {
        assert!(v0_view.get(count).is_none(), "{v0_view}");
    }
    for route in ["replies", "embeds"] {
        invalid_get_request(
            &format!("/v0/resource/{rid}/{route}"),
            StatusCode::NOT_FOUND,
        )
        .await?;
    }
    let page = get_request(&format!("/v1/resource/{rid}/replies?limit=1")).await?;
    assert_eq!(page.as_array().unwrap().len(), 1);
    assert_eq!(page[0]["details"]["id"], ids[1]);
    assert_eq!(page[0]["relationships"]["replied"], target);
    let page = get_request(&format!("/v1/resource/{rid}/replies?skip=1&limit=1")).await?;
    assert_eq!(page[0]["details"]["id"], ids[0]);
    assert_eq!(
        get_request(&format!("/v1/resource/{rid}/replies?skip=2")).await?,
        json!([])
    );
    let embeds = get_request(&format!("/v1/resource/{rid}/embeds")).await?;
    assert_eq!(embeds[0]["details"]["id"], ids[2]);
    assert_eq!(embeds[0]["relationships"]["embedded"], target);
    // `/v0` is frozen: a post's relationships have no `embedded`, and an external `replied`
    // reads `null`, as before these references, in the post view and the post stream alike;
    // `/v1` serves both.
    for (id, field) in [(&ids[1], "replied"), (&ids[2], "embedded")] {
        let v1_post = get_request(&format!("/v1/post/{author}/{id}")).await?;
        assert_eq!(v1_post["relationships"][field], target, "{v1_post}");
        let v0_post = get_request(&format!("/v0/post/{author}/{id}")).await?;
        assert!(
            v0_post["relationships"].get("embedded").is_none(),
            "{v0_post}"
        );
        assert_eq!(
            v0_post["relationships"]["replied"],
            Value::Null,
            "{v0_post}"
        );
    }
    let by_ids = json!({
        "post_ids": [format!("{author}:{}", ids[1]), format!("{author}:{}", ids[2])]
    });
    let v1_posts = post_request("/v1/stream/posts/by_ids", by_ids.clone()).await?;
    assert_eq!(
        v1_posts[0]["relationships"]["replied"], target,
        "{v1_posts}"
    );
    assert_eq!(
        v1_posts[1]["relationships"]["embedded"], target,
        "{v1_posts}"
    );
    let v0_posts = post_request("/v0/stream/posts/by_ids", by_ids).await?;
    assert_eq!(v0_posts.as_array().unwrap().len(), 2, "{v0_posts}");
    for post in v0_posts.as_array().unwrap() {
        assert!(post["relationships"].get("embedded").is_none(), "{post}");
        assert_eq!(post["relationships"]["replied"], Value::Null, "{post}");
    }
    for route in ["replies", "embeds"] {
        for query in [
            "limit=0",
            "limit=101",
            "skip=-1",
            "skip=10001",
            "viewer_id=invalid",
        ] {
            invalid_get_request(
                &format!("/v1/resource/{rid}/{route}?{query}"),
                StatusCode::BAD_REQUEST,
            )
            .await?;
        }
        invalid_get_request(
            &format!("/v1/resource/invalid/{route}"),
            StatusCode::BAD_REQUEST,
        )
        .await?;
        invalid_get_request(
            &format!("/v1/resource/00000000000000000000000000000000/{route}"),
            StatusCode::NOT_FOUND,
        )
        .await?;
    }
    graph
        .run(
            Query::new(
                "reference_api_tombstone",
                "MATCH (p:Post {id:$id}) SET p.deleted=true",
            )
            .param("id", ids[1].clone()),
        )
        .await?;
    let remaining = get_request(&format!("/v1/resource/{rid}/replies")).await?;
    assert_eq!(remaining.as_array().unwrap().len(), 1);
    for id in &ids {
        references::retract(&author, id).await?;
    }
    graph.run(Query::new("reference_api_cleanup", "MATCH (u:User {id:$author}) OPTIONAL MATCH (u)-[:AUTHORED]->(p:Post) DETACH DELETE p, u").param("author", author)).await?;
    graph
        .run(
            Query::new(
                "reference_api_resource_cleanup",
                "MATCH (r:Resource {id: $rid}) DETACH DELETE r",
            )
            .param("rid", rid),
        )
        .await?;
    Ok(())
}

/// `/v0` serves a Resource that only posts reference, with no tag, as an existing Resource:
/// the tag route and the lookup by URI answer 200 with no tags. A reply to a URL is a reply on
/// `/v0` too: the author's own root timeline leaves it out and their replies list it.
#[tokio_shared_rt::test(shared)]
async fn v0_serves_an_untagged_referenced_resource_and_a_url_reply_as_a_reply() -> Result<()> {
    crate::utils::server::TestServiceServer::get_test_server().await;
    let author = pubky::Keypair::random().public_key().to_z32();
    let target = format!("https://example.com/api-v0-untagged/{author}");
    let rid = resource_id(&target);
    let graph = get_neo4j_graph()?;
    graph
        .run(
            Query::new("v0_untagged_user", "CREATE (:User {id: $author})")
                .param("author", author.clone()),
        )
        .await?;
    let at = chrono::Utc::now().timestamp_millis();
    let stamp = chrono::Utc::now().timestamp_micros();
    // A root post, then a newer reply to the URL.
    let posts: Vec<PostDetails> = (0..2)
        .map(|n| {
            let id = format!("{:013X}", stamp + n);
            PostDetails {
                author: author.clone(),
                uri: format!("pubky://{author}/pub/social/v1/posts/{id}"),
                id,
                content: "v0 untagged".into(),
                indexed_at: at + n,
                kind: nexus_common::models::post::PostKind::Short,
                attachments: vec![],
                lock: None,
                deleted: false,
            }
        })
        .collect();
    let [root, reply] = [&posts[0], &posts[1]];
    for post in &posts {
        post.put_to_graph(None, None).await?;
    }
    references::sync(
        reply,
        vec![references::reference(
            &author,
            &format!("{}/{}.json", reply.uri, reply.id),
            &target,
            ResourceChangeKind::Reply,
        )?],
    )
    .await?;
    // As the watcher indexes a new post: the graph says which one is a reply.
    for post in &posts {
        post.put_to_index(&author, None, false, None).await?;
    }

    let view = get_request(&format!("/v0/resource/{rid}/tags")).await?;
    assert_eq!(view["resource"]["id"], rid.as_str(), "{view}");
    assert_eq!(view["tags"], json!([]), "{view}");
    let by_uri = get_request(&format!("/v0/resource/by-uri?uri={target}")).await?;
    assert_eq!(by_uri, view);

    let keys = |stream: &Value| -> Vec<String> {
        stream
            .as_array()
            .unwrap_or_else(|| panic!("not a stream: {stream}"))
            .iter()
            .map(|post| post["details"]["id"].as_str().unwrap().to_string())
            .collect()
    };
    // The author's own ALL timeline, which the trust filter does not hide from them.
    let window = format!("start={}&end={}", at + 1000, at - 1000);
    let timeline = get_request(&format!(
        "/v0/stream/posts?source=all&sorting=timeline&viewer_id={author}&{window}&limit=50"
    ))
    .await?;
    assert!(keys(&timeline).contains(&root.id), "{timeline}");
    assert!(!keys(&timeline).contains(&reply.id), "{timeline}");
    let own = get_request(&format!(
        "/v0/stream/posts?source=author&author_id={author}"
    ))
    .await?;
    assert_eq!(keys(&own), std::slice::from_ref(&root.id), "{own}");
    let replies = get_request(&format!(
        "/v0/stream/posts?source=author_replies&author_id={author}"
    ))
    .await?;
    assert_eq!(keys(&replies), std::slice::from_ref(&reply.id), "{replies}");
    assert_eq!(replies[0]["relationships"]["replied"], Value::Null);

    references::retract(&author, &reply.id).await?;
    for post in &posts {
        PostDetails::delete_from_index(&author, &post.id, None).await?;
        PostStream::remove_from_replies_per_user_sorted_set(&author, &post.id).await?;
    }
    graph.run(Query::new("v0_untagged_cleanup", "MATCH (u:User {id:$author}) OPTIONAL MATCH (u)-[:AUTHORED]->(p:Post) DETACH DELETE p, u").param("author", author)).await?;
    graph
        .run(
            Query::new(
                "v0_untagged_resource_cleanup",
                "MATCH (r:Resource {id: $rid}) DETACH DELETE r",
            )
            .param("rid", rid),
        )
        .await?;
    Ok(())
}
