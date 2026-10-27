use crate::utils::{get_request, invalid_get_request, post_request};
use anyhow::Result;
use axum::http::StatusCode;
use nexus_common::db::{get_neo4j_graph, graph::Query};
use nexus_common::models::event::resource_changes::ResourceChangeKind;
use nexus_common::models::post::PostDetails;
use nexus_common::models::resource::references;
use nexus_common::universal_tag::normalize::resource_id;
use serde_json::{json, Value};

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
        references::prepare_delete(&author, id).await?;
        references::finish_delete(&author, id).await?;
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
