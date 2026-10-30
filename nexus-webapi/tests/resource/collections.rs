//! External resources in collections and bookmarks: the collections listing a Resource, the
//! viewer's bookmark on the Resource view, and the stream of a user's bookmarked resources.

use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;
use nexus_common::db::{get_neo4j_graph, graph::Query};
use nexus_common::models::event::resource_changes::ResourceChangeKind;
use nexus_common::models::post::{PostDetails, PostKind};
use nexus_common::models::resource::bookmark::ResourceBookmark;
use nexus_common::models::resource::references;
use nexus_common::universal_tag::normalize::resource_id;
use serde_json::{json, Value};

async fn collection(author: &str, offset: i64, target: &str) -> Result<String> {
    let id = format!("{:013X}", chrono::Utc::now().timestamp_micros() + offset);
    let uri = format!("pubky://{author}/pub/social/v1/posts/{id}");
    let post = PostDetails {
        author: author.into(),
        id: id.clone(),
        uri: uri.clone(),
        content: json!({"name": "Reading list", "items": [{"uri": target}]}).to_string(),
        indexed_at: 1000 + offset,
        kind: PostKind::Collection,
        attachments: Some(vec![]),
        lock: None,
        deleted: false,
    };
    post.put_to_graph(None, None).await?;
    references::sync(
        &post,
        vec![references::reference(
            author,
            &format!("{uri}/{id}.json"),
            target,
            ResourceChangeKind::Collection,
        )?],
    )
    .await?;
    Ok(id)
}

fn ids(body: &Value) -> Vec<&str> {
    body.as_array()
        .expect("an array of posts")
        .iter()
        .filter_map(|post| post["details"]["id"].as_str())
        .collect()
}

#[tokio_shared_rt::test(shared)]
async fn resource_collections_bookmark_and_bookmark_stream() -> Result<()> {
    crate::utils::server::TestServiceServer::get_test_server().await;
    let alice = pubky::Keypair::random().public_key().to_z32();
    let bob = pubky::Keypair::random().public_key().to_z32();
    let target = format!("https://example.com/books/{alice}");
    let rid = resource_id(&target);
    let graph = get_neo4j_graph()?;
    for user in [&alice, &bob] {
        graph
            .run(
                Query::new("resource_collections_user", "CREATE (:User {id: $id})")
                    .param("id", user.clone()),
            )
            .await?;
    }
    let older = collection(&alice, 0, &target).await?;
    let newer = collection(&bob, 1, &target).await?;

    // Collections listing the resource, newest first, filtered by curator and paged.
    let base = format!("/v0/resource/{rid}/collections");
    let body = get_request(&base).await?;
    assert_eq!(ids(&body), [newer.as_str(), older.as_str()]);
    assert_eq!(body[0]["details"]["kind"], "collection");
    let body = get_request(&format!("{base}?curator_id={alice}")).await?;
    assert_eq!(ids(&body), [older.as_str()]);
    let body = get_request(&format!("{base}?skip=1&limit=1")).await?;
    assert_eq!(ids(&body), [older.as_str()]);
    let view = get_request(&format!("/v0/resource/{rid}/tags")).await?;
    assert_eq!(view["collections_count"], 2);
    for query in [
        "limit=0",
        "limit=101",
        "skip=10001",
        "curator_id=invalid",
        "viewer_id=invalid",
    ] {
        invalid_get_request(&format!("{base}?{query}"), StatusCode::BAD_REQUEST).await?;
    }
    invalid_get_request(
        "/v0/resource/00000000000000000000000000000000/collections",
        StatusCode::NOT_FOUND,
    )
    .await?;

    // The viewer's bookmark on the resource view, as a post view carries it.
    ResourceBookmark::put_to_graph(&alice, &rid, &target, "https", "BOOKMARK1", 4242).await?;
    let view = get_request(&format!("/v0/resource/{rid}/tags?viewer_id={alice}")).await?;
    assert_eq!(
        view["bookmark"],
        json!({"id": "BOOKMARK1", "indexed_at": 4242})
    );
    let view = get_request(&format!("/v0/resource/{rid}/tags?viewer_id={bob}")).await?;
    assert_eq!(view["bookmark"], Value::Null);
    let view = get_request(&format!("/v0/resource/{rid}/tags")).await?;
    assert_eq!(view["bookmark"], Value::Null);

    // The user's bookmarked resources, with the cursor of the bookmark time.
    let keys = get_request(&format!("/v0/stream/resources/ids?bookmarked_by={alice}")).await?;
    assert_eq!(keys["resource_ids"], json!([rid]));
    assert_eq!(keys["last_score"].as_f64(), Some(4242.0));
    let keys = get_request(&format!(
        "/v0/stream/resources/ids?bookmarked_by={alice}&end=4243&order=ascending"
    ))
    .await?;
    assert_eq!(keys["resource_ids"], json!([rid]));
    let keys = get_request(&format!(
        "/v0/stream/resources/ids?bookmarked_by={alice}&start=4241"
    ))
    .await?;
    assert_eq!(keys["resource_ids"], json!([]));
    let views = get_request(&format!(
        "/v0/stream/resources?bookmarked_by={alice}&viewer_id={alice}"
    ))
    .await?;
    assert_eq!(views[0]["details"]["id"], rid);
    assert_eq!(views[0]["bookmark"]["id"], "BOOKMARK1");
    let none = get_request(&format!("/v0/stream/resources/ids?bookmarked_by={bob}")).await?;
    assert_eq!(none["resource_ids"], json!([]));
    for query in [
        "tags=books",
        "app=mapky",
        "sorting=rating",
        "bookmarked_by=invalid",
    ] {
        invalid_get_request(
            &format!("/v0/stream/resources/ids?bookmarked_by={alice}&{query}"),
            StatusCode::BAD_REQUEST,
        )
        .await?;
    }

    // Removing the bookmark keeps the Resource the collections still reference.
    assert_eq!(
        ResourceBookmark::del_from_graph(&alice, "BOOKMARK1").await?,
        Some(rid.clone())
    );
    assert_eq!(
        ResourceBookmark::del_from_graph(&alice, "BOOKMARK1").await?,
        None
    );
    assert!(references::exists(&rid).await?);

    for (author, id) in [(&alice, &older), (&bob, &newer)] {
        references::prepare_delete(author, id).await?;
        references::finish_delete(author, id).await?;
    }
    assert!(!references::exists(&rid).await?);
    for user in [alice, bob] {
        graph.run(Query::new("resource_collections_cleanup", "MATCH (u:User {id:$id}) OPTIONAL MATCH (u)-[:AUTHORED]->(p:Post) DETACH DELETE p, u").param("id", user)).await?;
    }
    Ok(())
}
