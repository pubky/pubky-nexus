//! `GET /v0/post/{author_id}/{post_id}/items`: a Collection's items, posts and external
//! resources, in bounded pages with a cursor, past the spec's current 100-item limit.

use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;
use nexus_common::db::{get_neo4j_graph, graph::Query};
use nexus_common::models::event::resource_changes::ResourceChangeKind;
use nexus_common::models::post::{PostDetails, PostKind};
use nexus_common::models::resource::references;
use nexus_common::universal_tag::normalize::{normalize_uri, resource_id};
use serde_json::json;

async fn post(author: &str, offset: i64, kind: PostKind, content: String) -> Result<PostDetails> {
    // A canonical timestamp id: its last digit is even (no dangling bit), so the item
    // address parses as a social v1 post.
    let id = format!(
        "{:013X}",
        (chrono::Utc::now().timestamp_micros() + 2 * offset) & !1
    );
    let post = PostDetails {
        author: author.into(),
        uri: format!("pubky://{author}/pub/social/v1/posts/{id}"),
        id,
        content,
        indexed_at: 1000 + offset,
        kind,
        attachments: Some(vec![]),
        lock: None,
        deleted: false,
    };
    post.put_to_graph(None, None).await?;
    Ok(post)
}

#[tokio_shared_rt::test(shared)]
async fn collection_items_page_past_one_hundred_items() -> Result<()> {
    crate::utils::server::TestServiceServer::get_test_server().await;
    let author = pubky::Keypair::random().public_key().to_z32();
    let graph = get_neo4j_graph()?;
    graph
        .run(
            Query::new("collection_items_user", "CREATE (:User {id: $id})")
                .param("id", author.clone()),
        )
        .await?;
    // 250 items, every fifth a Pubky post: written straight to the graph, since the
    // pinned spec still rejects an envelope over 100 items at the watcher.
    let item_post = post(&author, 0, PostKind::Short, "item".into()).await?;
    let uris: Vec<String> = (0..250)
        .map(|n| match n % 5 {
            0 => item_post.uri.clone(),
            _ => format!("https://example.com/{author}/books/{n}"),
        })
        .collect();
    let items: Vec<_> = uris.iter().map(|uri| json!({"uri": uri})).collect();
    let content = json!({"name": "Long list", "items": items}).to_string();
    let collection = post(&author, 1, PostKind::Collection, content).await?;
    let source = format!("{}/{}.json", collection.uri, collection.id);
    // Every external item is a COLLECTED reference, as the watcher would write it.
    let lines = uris
        .iter()
        .filter(|uri| uri.starts_with("https://"))
        .map(|uri| references::reference(&author, &source, uri, ResourceChangeKind::Collection))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(lines.len(), 200);
    references::sync(&collection, lines).await?;

    let base = format!("/v0/post/{author}/{}/items", collection.id);
    let mut cursor: Option<String> = None;
    let mut seen = vec![];
    let mut pages = 0;
    loop {
        let path = match &cursor {
            Some(cursor) => format!("{base}?limit=100&cursor={cursor}"),
            None => format!("{base}?limit=100"),
        };
        let page = get_request(&path).await?;
        let items = page["items"].as_array().unwrap().clone();
        assert!(items.len() <= 100);
        seen.extend(items);
        pages += 1;
        match page["next_cursor"].as_str() {
            Some(next) => cursor = Some(next.to_string()),
            None => break,
        }
    }
    assert_eq!((pages, seen.len()), (3, 250));
    for (n, item) in seen.iter().enumerate() {
        assert_eq!(item["position"], n);
        assert_eq!(item["uri"], uris[n]);
    }
    assert_eq!(seen[0]["post_key"], format!("{author}:{}", item_post.id));
    assert_eq!(seen[0]["resource_id"], json!(null));
    let external = seen[249]["resource_id"].as_str().unwrap().to_string();
    assert_eq!(
        external,
        resource_id(&normalize_uri(&uris[249]).map_err(anyhow::Error::msg)?.0)
    );
    // The item's Resource knows the collection, past the 100th item too.
    let view = get_request(&format!("/v0/resource/{external}/tags")).await?;
    assert_eq!(view["collections_count"], 1);

    // Default page size, bounds and errors.
    let page = get_request(&base).await?;
    assert_eq!(page["items"].as_array().unwrap().len(), 20);
    assert_eq!(page["next_cursor"], "20");
    for query in ["limit=0", "limit=101", "cursor=abc", "cursor=-1"] {
        invalid_get_request(&format!("{base}?{query}"), StatusCode::BAD_REQUEST).await?;
    }
    invalid_get_request(
        &format!("/v0/post/{author}/{}/items", item_post.id),
        StatusCode::NOT_FOUND,
    )
    .await?;
    invalid_get_request(
        &format!("/v0/post/{author}/0000000000000/items"),
        StatusCode::NOT_FOUND,
    )
    .await?;

    references::prepare_delete(&author, &collection.id).await?;
    references::finish_delete(&author, &collection.id).await?;
    graph.run(Query::new("collection_items_cleanup", "MATCH (u:User {id:$id}) OPTIONAL MATCH (u)-[:AUTHORED]->(p:Post) DETACH DELETE p, u").param("id", author)).await?;
    Ok(())
}
