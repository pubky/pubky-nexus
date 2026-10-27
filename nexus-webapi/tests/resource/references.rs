use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;
use nexus_common::db::{get_neo4j_graph, graph::Query};
use nexus_common::models::event::resource_changes::ResourceChangeKind;
use nexus_common::models::post::PostDetails;
use nexus_common::models::resource::references;
use nexus_common::universal_tag::normalize::resource_id;
use serde_json::json;

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
            attachments: Some(vec![]),
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
    let view = get_request(&format!("/v0/resource/{rid}/tags")).await?;
    assert_eq!(
        (
            view["replies_count"].as_u64(),
            view["embeds_count"].as_u64(),
            view["collections_count"].as_u64()
        ),
        (Some(2), Some(1), Some(0))
    );
    let page = get_request(&format!("/v0/resource/{rid}/replies?limit=1")).await?;
    assert_eq!(page.as_array().unwrap().len(), 1);
    assert_eq!(page[0]["details"]["id"], ids[1]);
    assert_eq!(page[0]["relationships"]["replied"], target);
    let page = get_request(&format!("/v0/resource/{rid}/replies?skip=1&limit=1")).await?;
    assert_eq!(page[0]["details"]["id"], ids[0]);
    assert_eq!(
        get_request(&format!("/v0/resource/{rid}/replies?skip=2")).await?,
        json!([])
    );
    let embeds = get_request(&format!("/v0/resource/{rid}/embeds")).await?;
    assert_eq!(embeds[0]["details"]["id"], ids[2]);
    assert_eq!(embeds[0]["relationships"]["embedded"], target);
    for route in ["replies", "embeds"] {
        for query in [
            "limit=0",
            "limit=101",
            "skip=-1",
            "skip=10001",
            "viewer_id=invalid",
        ] {
            invalid_get_request(
                &format!("/v0/resource/{rid}/{route}?{query}"),
                StatusCode::BAD_REQUEST,
            )
            .await?;
        }
        invalid_get_request(
            &format!("/v0/resource/invalid/{route}"),
            StatusCode::BAD_REQUEST,
        )
        .await?;
        invalid_get_request(
            &format!("/v0/resource/00000000000000000000000000000000/{route}"),
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
    let remaining = get_request(&format!("/v0/resource/{rid}/replies")).await?;
    assert_eq!(remaining.as_array().unwrap().len(), 1);
    for id in &ids {
        references::prepare_delete(&author, id).await?;
        references::finish_delete(&author, id).await?;
    }
    graph.run(Query::new("reference_api_cleanup", "MATCH (u:User {id:$author}) OPTIONAL MATCH (u)-[:AUTHORED]->(p:Post) DETACH DELETE p, u").param("author", author)).await?;
    Ok(())
}
