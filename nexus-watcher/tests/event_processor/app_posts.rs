use super::posts::utils::test_user;
use super::utils::watcher::{generate_post_id, retrieve_and_handle_event_line, WatcherTest};
use anyhow::Result;
use nexus_common::db::{fetch_row_from_graph, graph::Query};
use nexus_common::models::event::resource_changes::{
    ResourceChange, ResourceChangeKind, ResourceChangeOp,
};
use nexus_common::models::resource::view::ResourceView;
use nexus_common::universal_tag::normalize::resource_id;
use nexus_watcher::EventProcessorError;
use pubky::{Keypair, ResourcePath};
use pubky_social_specs::{
    traits::{HasIdPath, HashId},
    PubkySocialTag,
};
use serde_json::json;

#[tokio_shared_rt::test(shared)]
async fn generic_versions_publish_only_self_references_and_preserve_social_references() -> Result<()>
{
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Generic app post", ""))
        .await?;
    let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
    let mut paths = vec![];
    for (namespace, kind) in [
        ("maps", "route"),
        ("events", "availability"),
        ("music", "listening"),
        ("music", "listen"),
        ("example-2", "future_kind"),
    ] {
        let id = generate_post_id();
        let path: ResourcePath = format!("/pub/{namespace}/v1/posts/{id}/{id}.json").parse()?;
        let uri = format!("pubky://{author}/pub/{namespace}/v1/posts/{id}");
        test.put(
            &key,
            &path,
            &json!({"kind":kind, "content":"opaque app content", "ext":{"x":true}}),
        )
        .await?;
        let row = fetch_row_from_graph(Query::new("test_app_no_graph", "OPTIONAL MATCH (p:Post {id:$id}) WITH count(p) AS posts OPTIONAL MATCH (r:Resource {id:$rid}) RETURN posts, count(r) AS resources").param("id", id).param("rid", resource_id(&uri))).await?.unwrap();
        assert_eq!(row.get::<i64>("posts")?, 0);
        assert_eq!(row.get::<i64>("resources")?, 0);
        paths.push((path, uri));
    }
    let (first, uri) = &paths[0];
    retrieve_and_handle_event_line(
        &format!("PUT pubky://{author}{first}"),
        test.event_processor_runner.event_handler.clone(),
    )
    .await?;
    let tag = PubkySocialTag::new(uri.clone(), "place".into());
    let tag_path: ResourcePath = PubkySocialTag::create_path(&tag.create_id()).parse()?;
    test.put(&key, &tag_path, &tag).await?;
    let reply = generate_post_id();
    let reply_path: ResourcePath = format!("/pub/social/v1/posts/{reply}/{reply}.json").parse()?;
    test.put(
        &key,
        &reply_path,
        &json!({"kind":"note", "content":"discuss", "parent":uri}),
    )
    .await?;
    let rid = resource_id(uri);
    let view = ResourceView::get_by_id(&rid, None, None, None, None)
        .await?
        .unwrap();
    assert_eq!((view.replies_count, view.taggers_count), (1, 1));
    for (path, _) in &paths {
        test.del(&key, path).await?;
    }
    retrieve_and_handle_event_line(
        &format!("DEL pubky://{author}{first}"),
        test.event_processor_runner.event_handler.clone(),
    )
    .await?;
    let view = ResourceView::get_by_id(&rid, None, None, None, None)
        .await?
        .unwrap();
    assert_eq!(
        (view.replies_count, view.taggers_count),
        (1, 1),
        "app deletes do not own shared Resources"
    );
    let page = ResourceChange::get_from_index(cursor, 1000).await?.0;
    let own: Vec<_> = page
        .lines
        .iter()
        .filter(|l| l.author == author && l.kind == ResourceChangeKind::Post)
        .collect();
    assert_eq!(own.len(), 12, "five puts, replay, five dels, replay");
    for (path, uri) in &paths {
        assert!(own.iter().any(|l| l.op == ResourceChangeOp::Put
            && l.uri == *uri
            && l.object_path == path.to_string()
            && l.resource_id == resource_id(uri)));
    }
    let mut consumer = std::collections::HashSet::new();
    for line in own {
        let replay_key = (&line.author, &line.object_path, &line.resource_id);
        match line.op {
            ResourceChangeOp::Put => {
                consumer.insert(replay_key);
            }
            ResourceChangeOp::Del => {
                consumer.remove(&replay_key);
            }
        }
    }
    assert!(consumer.is_empty());
    test.del(&key, &tag_path).await?;
    test.del(&key, &reply_path).await?;
    assert!(ResourceView::get_by_id(&rid, None, None, None, None)
        .await?
        .is_none());
    test.cleanup_user(&key).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn invalid_envelope_and_unstored_paths_do_not_publish() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Invalid app post", ""))
        .await?;
    test = test.remove_event_processing().await;
    let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/example/v1/posts/{id}/{id}.json").parse()?;
    test.put(&key, &path, &json!({"kind":"anything", "content":{}}))
        .await?;
    let error = retrieve_and_handle_event_line(
        &format!("PUT pubky://{author}{path}"),
        test.event_processor_runner.event_handler.clone(),
    )
    .await
    .unwrap_err();
    assert!(matches!(error, EventProcessorError::SpecValidation(_)));
    for uri in [
        format!("pubky://{author}/pub/example/v1/posts/{id}"),
        format!("pubky://{author}/priv/example/v1/posts/{id}/{id}.json"),
    ] {
        let _ = retrieve_and_handle_event_line(
            &format!("DEL {uri}"),
            test.event_processor_runner.event_handler.clone(),
        )
        .await;
    }
    assert!(!ResourceChange::get_from_index(cursor, 1000)
        .await?
        .0
        .lines
        .iter()
        .any(|line| line.author == author));
    test.cleanup_user(&key).await?;
    Ok(())
}
