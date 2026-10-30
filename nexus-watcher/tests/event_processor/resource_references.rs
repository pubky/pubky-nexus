use super::posts::utils::test_user;
use super::utils::watcher::{generate_post_id, retrieve_and_handle_event_line, WatcherTest};
use anyhow::Result;
use nexus_common::db::{get_neo4j_graph, graph::Query, RedisOps};
use nexus_common::models::event::resource_changes::{
    ResourceChange, ResourceChangeKind, ResourceChangeOp,
};
use nexus_common::models::post::{PostDetails, PostRelationships};
use nexus_common::models::resource::{references, view::ResourceView};
use nexus_common::universal_tag::normalize::{normalize_uri, resource_id};
use pubky::{Keypair, ResourcePath};
use pubky_social_specs::{
    traits::{HasIdPath, HashId},
    PubkySocialTag,
};
use serde_json::json;

#[tokio_shared_rt::test(shared)]
async fn replies_embeds_and_collections_share_resources_and_replay_keys() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("External references", ""))
        .await?;
    let target = format!("https://example.com/references/{author}");
    let second_target = format!("https://example.com/second/{author}");
    let rid = resource_id(&target);
    let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
    let tag = PubkySocialTag::new(
        target.replace("example.com", "EXAMPLE.com"),
        "shared".into(),
    );
    let tag_path: ResourcePath = PubkySocialTag::create_path(&tag.create_id()).parse()?;
    test.put(&key, &tag_path, &tag).await?;
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    test.put(
        &key,
        &path,
        &json!({"kind":"note","content":"reply and embed","parent":target,"embed":target}),
    )
    .await?;
    let collection_id = generate_post_id();
    let collection: ResourcePath =
        format!("/pub/social/v1/posts/{collection_id}/{collection_id}.json").parse()?;
    test.put(&key, &collection, &json!({"kind":"collection", "content":json!({"name":"Resources","items":[{"uri":target},{"uri":second_target}]}).to_string()})).await?;
    let view = ResourceView::get_by_id(&rid, None, None, None, None)
        .await?
        .unwrap();
    assert_eq!(
        (
            view.replies_count,
            view.embeds_count,
            view.collections_count,
            view.taggers_count
        ),
        (1, 1, 1, 1)
    );
    assert_eq!(
        references::posts(&rid, ResourceChangeKind::Reply, 0, 10).await?,
        [(author.clone(), id.clone())]
    );
    assert_eq!(
        references::posts(&rid, ResourceChangeKind::Embed, 0, 10).await?,
        [(author.clone(), id.clone())]
    );
    assert_eq!(
        PostRelationships::get_from_graph(&author, &id)
            .await?
            .unwrap()
            .replied
            .as_deref(),
        Some(target.as_str())
    );
    // Graph recovery must not turn a Resource reply into a root timeline post.
    PostDetails::remove_from_index_multiple_json(&[&[author.as_str(), id.as_str()]]).await?;
    retrieve_and_handle_event_line(
        &format!("PUT pubky://{author}{path}"),
        test.event_processor_runner.event_handler.clone(),
    )
    .await?;
    // A tag deletion must preserve the node while posts still reference it.
    test.del(&key, &tag_path).await?;
    assert!(ResourceView::get_by_id(&rid, None, None, None, None)
        .await?
        .is_some());
    let edit = generate_post_id();
    let collection_edit: ResourcePath =
        format!("/pub/social/v1/posts/{collection_id}/{edit}.json").parse()?;
    test.put(&key, &collection_edit, &json!({"kind":"collection", "content":json!({"name":"Changed","items":[{"uri":second_target}]}).to_string()})).await?;
    assert_eq!(
        ResourceView::get_by_id(&rid, None, None, None, None)
            .await?
            .unwrap()
            .collections_count,
        0
    );
    // External targets remain editable, as before Resource projection existed.
    let reply_edit_id = generate_post_id();
    let reply_edit: ResourcePath =
        format!("/pub/social/v1/posts/{id}/{reply_edit_id}.json").parse()?;
    test.put(
        &key,
        &reply_edit,
        &json!({"kind":"note","content":"moved", "parent":second_target, "embed":second_target}),
    )
    .await?;
    assert!(ResourceView::get_by_id(&rid, None, None, None, None)
        .await?
        .is_none());
    assert_eq!(
        PostRelationships::get_from_graph(&author, &id)
            .await?
            .unwrap()
            .replied
            .as_deref(),
        Some(second_target.as_str())
    );
    // Removing the external parent restores root timeline membership.
    test.put(
        &key,
        &reply_edit,
        &json!({"kind":"note","content":"root again"}),
    )
    .await?;
    assert!(PostRelationships::get_from_graph(&author, &id)
        .await?
        .unwrap()
        .replied
        .is_none());
    assert!(
        super::posts::utils::check_member_global_timeline_user_post(&author, &id)
            .await?
            .is_some()
    );
    test.del(&key, &reply_edit).await?;
    assert!(ResourceView::get_by_id(&rid, None, None, None, None)
        .await?
        .is_none());
    test.del(&key, &collection_edit).await?;
    assert!(
        ResourceView::get_by_id(&resource_id(&second_target), None, None, None, None)
            .await?
            .is_none()
    );
    let page = ResourceChange::get_from_index(cursor, 1000).await?.0;
    let own: Vec<_> = page
        .lines
        .iter()
        .filter(|line| line.author == author)
        .collect();
    assert!(own.iter().any(|line| line.kind == ResourceChangeKind::Reply
        && line.op == ResourceChangeOp::Put
        && line.object_path == path.to_string()));
    let first_collection: Vec<_> = own
        .iter()
        .filter(|line| {
            line.kind == ResourceChangeKind::Collection
                && line.op == ResourceChangeOp::Put
                && line.object_path == collection.to_string()
        })
        .collect();
    assert_eq!(
        first_collection.len(),
        2,
        "one file names two distinct targets"
    );
    let mut consumer = std::collections::BTreeMap::new();
    for line in own {
        let key = (
            line.author.clone(),
            line.object_path.clone(),
            format!("{:?}", line.kind),
            line.resource_id.clone(),
        );
        match line.op {
            ResourceChangeOp::Put => {
                consumer.insert(key, line.uri.clone());
            }
            ResourceChangeOp::Del => {
                consumer.remove(&key);
            }
        }
    }
    assert!(
        consumer.is_empty(),
        "deletes balance every reference, including edited collection versions"
    );
    test.cleanup_user(&key).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn graph_commit_before_append_replays_put_and_deleted_graph_replays_del() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Reference crash replay", ""))
        .await?;
    let target = format!("https://example.com/crash/{author}");
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    let envelope = json!({"kind":"note","content":"first","parent":target});
    test.put(&key, &path, &envelope).await?;
    test = test.remove_event_processing().await;
    let edit = generate_post_id();
    let version: ResourcePath = format!("/pub/social/v1/posts/{id}/{edit}.json").parse()?;
    test.put(&key, &version, &envelope).await?;
    let line = references::reference(
        &author,
        &format!("pubky://{author}{version}"),
        &target,
        ResourceChangeKind::Reply,
    )?;
    let post = PostDetails::get_by_id(&author, &id).await?.unwrap();
    let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
    references::stage(&post, vec![line]).await?; // committed graph and outbox, no append
    assert!(ResourceChange::get_from_index(cursor, 1000)
        .await?
        .0
        .lines
        .iter()
        .all(|line| line.author != author));
    let handler = test.event_processor_runner.event_handler.clone();
    retrieve_and_handle_event_line(&format!("PUT pubky://{author}{version}"), handler.clone())
        .await?;
    let lines = ResourceChange::get_from_index(cursor, 1000).await?.0.lines;
    let puts: Vec<_> = lines
        .iter()
        .filter(|line| {
            line.author == author
                && line.op == ResourceChangeOp::Put
                && line.object_path == version.to_string()
        })
        .collect();
    assert_eq!(
        puts.len(),
        2,
        "the outbox and handler retry both deliver the same reference key"
    );
    assert_eq!(
        (puts[0].resource_id.as_str(), puts[0].kind),
        (puts[1].resource_id.as_str(), puts[1].kind)
    );
    references::prepare_delete(&author, &id).await?;
    get_neo4j_graph()?.run(Query::new("reference_crash_delete", "MATCH (p:Post {id:$id}) WHERE EXISTS { (:User {id:$author})-[:AUTHORED]->(p) } DETACH DELETE p").param("id", id.clone()).param("author", author.clone())).await?;
    let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
    retrieve_and_handle_event_line(&format!("DEL pubky://{author}{version}"), handler.clone())
        .await?;
    retrieve_and_handle_event_line(&format!("DEL pubky://{author}{version}"), handler).await?;
    let lines = ResourceChange::get_from_index(cursor, 1000).await?.0.lines;
    let deletes: Vec<_> = lines
        .iter()
        .filter(|line| line.author == author && line.op == ResourceChangeOp::Del)
        .collect();
    assert_eq!(deletes.len(), 1);
    assert_eq!(deletes[0].object_path, version.to_string());
    assert!(ResourceView::get_by_id(
        &resource_id(&normalize_uri(&target).map_err(anyhow::Error::msg)?.0),
        None,
        None,
        None,
        None
    )
    .await?
    .is_none());
    test.cleanup_user(&key).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn tombstones_release_external_targets_and_cached_relationships() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Reference tombstone", ""))
        .await?;
    let target = format!("https://example.com/tombstone/{author}");
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    test.put(
        &key,
        &path,
        &json!({"kind":"note","content":"resource reply","parent":target}),
    )
    .await?;
    assert!(PostRelationships::get_by_id(&author, &id)
        .await?
        .unwrap()
        .replied
        .is_some());
    let child_id = generate_post_id();
    let child: ResourcePath = format!("/pub/social/v1/posts/{child_id}/{child_id}.json").parse()?;
    test.put(&key, &child, &json!({"kind":"note","content":"keep the parent tombstone","parent":format!("pubky://{author}/pub/social/v1/posts/{id}")})).await?;
    test.del(&key, &path).await?;
    assert!(PostDetails::get_by_id(&author, &id).await?.unwrap().deleted);
    assert!(PostRelationships::get_by_id(&author, &id)
        .await?
        .unwrap()
        .replied
        .is_none());
    assert!(
        ResourceView::get_by_id(&resource_id(&target), None, None, None, None)
            .await?
            .is_none()
    );
    PostDetails::remove_from_index_multiple_json(&[&[author.as_str(), id.as_str()]]).await?;
    PostDetails::reindex(&author, &id).await?;
    assert!(
        super::posts::utils::check_member_global_timeline_user_post(&author, &id)
            .await?
            .is_none()
    );
    assert!(
        super::posts::utils::check_member_user_post_timeline(&author, &id)
            .await?
            .is_none()
    );
    assert!(
        super::posts::utils::check_member_user_replies_timeline(&author, &id)
            .await?
            .is_some()
    );
    test.del(&key, &child).await?;
    test.cleanup_user(&key).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn hard_deleted_external_reply_leaves_author_replies() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Hard delete external reply", ""))
        .await?;
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    test.put(&key, &path, &json!({"kind":"note", "content":"reply", "parent":format!("https://example.com/hard-delete/{author}")})).await?;
    assert!(
        super::posts::utils::check_member_user_replies_timeline(&author, &id)
            .await?
            .is_some()
    );
    test.del(&key, &path).await?;
    assert!(PostDetails::get_from_graph(&author, &id).await?.is_none());
    assert!(
        super::posts::utils::check_member_user_replies_timeline(&author, &id)
            .await?
            .is_none()
    );
    test.cleanup_user(&key).await?;
    Ok(())
}
