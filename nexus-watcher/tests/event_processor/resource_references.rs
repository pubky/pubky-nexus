use super::posts::utils::v1_test_user;
use super::utils::watcher::{generate_post_id, retrieve_and_handle_event_line, WatcherTest};
use anyhow::Result;
use futures::TryStreamExt;
use nexus_common::db::kv::SortOrder;
use nexus_common::db::{get_neo4j_graph, graph::Query, queries, RedisOps};
use nexus_common::models::event::resource_changes::{
    ResourceChange, ResourceChangeKind, ResourceChangeOp,
};
use nexus_common::models::post::{
    KindFilter, PostCounts, PostDetails, PostKind, PostRelationships, StreamSource,
};
use nexus_common::models::resource::{references, view::ResourceView};
use nexus_common::models::user::UserCounts;
use nexus_common::types::{Pagination, StreamSorting};
use nexus_common::universal_tag::normalize::{normalize_uri, resource_id};
use nexus_common::utils::test_utils::{pause_references, ReferencePause};
use pubky::{Keypair, ResourcePath};
use pubky_social_specs::{
    traits::{HasIdPath, HashId},
    PubkySocialTag,
};
use serde_json::json;
use std::time::Duration;

#[tokio_shared_rt::test(shared)]
async fn replies_embeds_and_collections_share_resources_and_replay_keys() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("External references", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
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
    test.del(&key, &profile).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn graph_commit_before_append_replays_put_and_deleted_graph_replays_del() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Reference crash replay", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
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
    test.del(&key, &profile).await?;
    Ok(())
}

/// Handles `line` on its own task, as the live runners and the retry processor do.
fn spawn_event_line(
    line: String,
    handler: std::sync::Arc<dyn nexus_watcher::events::EventHandler>,
) -> tokio::task::JoinHandle<Result<(), nexus_watcher::EventProcessorError>> {
    tokio::spawn(async move { retrieve_and_handle_event_line(&line, handler).await })
}

/// Live events and retries of one post run in parallel. A PUT can read the outbox a lost append
/// left and pause before appending it, while a DEL of the post publishes that outbox, deletes the
/// post and retracts its references: the PUT's append after that would bring the deleted
/// reference back for feed readers. The post lock orders the two, so the feed ends with the
/// retraction.
#[tokio_shared_rt::test(shared)]
async fn concurrent_delete_is_not_overtaken_by_an_outbox_append() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Reference delete race", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
    let target = format!("https://example.com/delete-race/{author}");
    let rid = resource_id(&normalize_uri(&target).map_err(anyhow::Error::msg)?.0);
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    test.put(
        &key,
        &path,
        &json!({"kind":"note","content":"raced","parent":target}),
    )
    .await?;
    // A committed graph write whose feed append was lost leaves its outbox on the post.
    let line = references::reference(
        &author,
        &format!("pubky://{author}{path}"),
        &target,
        ResourceChangeKind::Reply,
    )?;
    let post = PostDetails::get_by_id(&author, &id).await?.unwrap();
    references::stage(&post, vec![line]).await?;
    let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;

    let handler = test.event_processor_runner.event_handler.clone();
    let (reached, release) = pause_references(ReferencePause::Flush, &author);
    let put = spawn_event_line(format!("PUT pubky://{author}{path}"), handler.clone());
    // The PUT read the outbox, and has not appended it.
    tokio::time::timeout(Duration::from_secs(30), reached)
        .await
        .map_err(|_| anyhow::anyhow!("the PUT never reached the outbox append"))??;
    let mut del = spawn_event_line(format!("DEL pubky://{author}{path}"), handler);
    // Unordered, the DEL runs to its end while the PUT is held.
    let early = tokio::time::timeout(Duration::from_secs(3), &mut del).await;
    let _ = release.send(());
    let put_result = put.await?;
    let del_result = match early {
        Ok(joined) => joined?,
        Err(_) => del.await?,
    };

    let ops: Vec<_> = ResourceChange::get_from_index(cursor, 1000)
        .await?
        .0
        .lines
        .iter()
        .filter(|line| line.author == author && line.resource_id == rid)
        .map(|line| line.op == ResourceChangeOp::Put)
        .collect();
    assert_eq!(
        ops.last(),
        Some(&false),
        "the delete's retraction is the reference's last change (true = put): {ops:?}"
    );
    put_result?;
    del_result?;
    assert!(ResourceView::get_by_id(&rid, None, None, None, None)
        .await?
        .is_none());
    test.del(&key, &profile).await?;
    Ok(())
}

/// An older version's PUT that wrote the post and paused before its references must not, once
/// a newer version's PUT finished, replace the newer version's references with its own: both
/// versions have the same content and kind, so the post write tells them apart. The post lock
/// orders the two handlers, and the post keeps the references of the version handled last.
#[tokio_shared_rt::test(shared)]
async fn older_version_does_not_replace_a_newer_versions_references() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Reference edit race", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
    let old_target = format!("https://example.com/older/{author}");
    let new_target = format!("https://example.com/newer/{author}");
    let id = generate_post_id();
    let older: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    test.put(
        &key,
        &older,
        &json!({"kind":"note","content":"same words","parent":old_target}),
    )
    .await?;
    let edit = generate_post_id();
    let newer: ResourcePath = format!("/pub/social/v1/posts/{id}/{edit}.json").parse()?;
    test.put(
        &key,
        &newer,
        &json!({"kind":"note","content":"same words","parent":new_target}),
    )
    .await?;

    let handler = test.event_processor_runner.event_handler.clone();
    let (reached, release) = pause_references(ReferencePause::Stage, &author);
    let old_put = spawn_event_line(format!("PUT pubky://{author}{older}"), handler.clone());
    // The older version's post write is done, its references are not.
    tokio::time::timeout(Duration::from_secs(30), reached)
        .await
        .map_err(|_| anyhow::anyhow!("the older PUT never reached its references"))??;
    let mut new_put = spawn_event_line(format!("PUT pubky://{author}{newer}"), handler);
    // Unordered, the newer PUT runs to its end while the older one is held.
    let early = tokio::time::timeout(Duration::from_secs(3), &mut new_put).await;
    let _ = release.send(());
    old_put.await??;
    match early {
        Ok(joined) => joined??,
        Err(_) => new_put.await??,
    }

    assert_eq!(
        PostRelationships::get_from_graph(&author, &id)
            .await?
            .unwrap()
            .replied
            .as_deref(),
        Some(new_target.as_str()),
        "the version handled last keeps its references"
    );
    assert!(
        references::posts(&resource_id(&old_target), ResourceChangeKind::Reply, 0, 10)
            .await?
            .is_empty()
    );
    test.del(&key, &newer).await?;
    test.del(&key, &profile).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn tombstones_release_external_targets_and_cached_relationships() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Reference tombstone", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
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
    test.del(&key, &profile).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn hard_deleted_external_reply_leaves_author_replies() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Hard delete external reply", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
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
    test.del(&key, &profile).await?;
    Ok(())
}

/// A tombstoned external reply keeps its place in the author's replies. The DEL that later
/// hard-deletes it, once the child that kept the tombstone is gone (a replayed DEL, or the DEL
/// of another of its version files), must take it out of the author's replies too.
#[tokio_shared_rt::test(shared)]
async fn hard_delete_of_a_tombstoned_external_reply_leaves_author_replies() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Tombstoned external reply", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    test.put(&key, &path, &json!({"kind":"note", "content":"reply", "parent":format!("https://example.com/tombstoned-reply/{author}")})).await?;
    let child_id = generate_post_id();
    let child: ResourcePath = format!("/pub/social/v1/posts/{child_id}/{child_id}.json").parse()?;
    test.put(&key, &child, &json!({"kind":"note","content":"keep the parent tombstone","parent":format!("pubky://{author}/pub/social/v1/posts/{id}")})).await?;
    test.del(&key, &path).await?;
    assert!(PostDetails::get_by_id(&author, &id).await?.unwrap().deleted);
    assert!(
        super::posts::utils::check_member_user_replies_timeline(&author, &id)
            .await?
            .is_some(),
        "a tombstone stays in the author's replies"
    );

    test.del(&key, &child).await?;
    retrieve_and_handle_event_line(
        &format!("DEL pubky://{author}{path}"),
        test.event_processor_runner.event_handler.clone(),
    )
    .await?;
    assert!(PostDetails::get_from_graph(&author, &id).await?.is_none());
    assert!(
        super::posts::utils::check_member_user_replies_timeline(&author, &id)
            .await?
            .is_none(),
        "a hard-deleted tombstone stays in the author's replies"
    );
    test.del(&key, &profile).await?;
    Ok(())
}

/// A tombstoned external reply lost its Resource reply edge but is still a reply, as the
/// author's replies set keeps it. The graph reads must classify it so too: a kind-filtered author
/// stream lists root posts only, the ranking refresh's entries and the post's counts call it a
/// reply, and the author's reply count keeps it.
#[tokio_shared_rt::test(shared)]
async fn tombstoned_external_reply_stays_out_of_root_graph_reads() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Tombstone root reads", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
    let root_id = generate_post_id();
    let root: ResourcePath = format!("/pub/social/v1/posts/{root_id}/{root_id}.json").parse()?;
    test.put(&key, &root, &json!({"kind":"note","content":"a root post"}))
        .await?;
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    test.put(&key, &path, &json!({"kind":"note", "content":"reply", "parent":format!("https://example.com/tombstone-root-reads/{author}")})).await?;
    let child_id = generate_post_id();
    let child: ResourcePath = format!("/pub/social/v1/posts/{child_id}/{child_id}.json").parse()?;
    test.put(&key, &child, &json!({"kind":"note","content":"keep the parent tombstone","parent":format!("pubky://{author}/pub/social/v1/posts/{id}")})).await?;
    test.del(&key, &path).await?;
    assert!(PostDetails::get_by_id(&author, &id).await?.unwrap().deleted);

    let stream = queries::get::post_stream(
        StreamSource::Author {
            author_id: author.clone(),
        },
        StreamSorting::Timeline,
        SortOrder::Descending,
        &None,
        Pagination::default(),
        Some(KindFilter::Kind(PostKind::Short)),
        None,
    )?;
    let mut rows = get_neo4j_graph()?.execute(stream).await?;
    let mut streamed = Vec::new();
    while let Some(row) = rows.try_next().await? {
        streamed.push(row.get::<String>("post_id")?);
    }
    assert_eq!(streamed, std::slice::from_ref(&root_id), "root posts only");

    let mut rows = get_neo4j_graph()?
        .execute(queries::get::author_post_entries(
            &author,
            (i64::MIN, ""),
            100,
        ))
        .await?;
    let mut entries = std::collections::BTreeMap::new();
    while let Some(row) = rows.try_next().await? {
        entries.insert(row.get::<String>("post_id")?, row.get::<bool>("is_reply")?);
    }
    assert_eq!(
        entries,
        [(root_id, false), (id.clone(), true), (child_id, true)].into(),
        "the ranking refresh keeps the tombstone out of the root sets"
    );
    assert!(PostCounts::get_from_graph(&author, &id).await?.unwrap().1);
    assert_eq!(
        UserCounts::get_by_id(&author).await?.unwrap().replies,
        2,
        "the tombstone and the child are both replies"
    );

    // The reply file is already deleted: the tombstone stays, as in the tests above.
    test.del(&key, &child).await?;
    test.del(&key, &root).await?;
    test.del(&key, &profile).await?;
    Ok(())
}

/// A native reply that also embeds an external URI, tombstoned because a child replies to it,
/// keeps its cached relationships: they are the gate its later hard delete reads before it takes
/// the reply's engagement off the parent. Finishing the tombstone's reference journal must not
/// drop them, and the hard delete must bring the parent's engagement back down.
#[tokio_shared_rt::test(shared)]
async fn tombstoned_reply_with_an_external_embed_keeps_its_hard_delete_gate() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Tombstoned embedding reply", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
    let root_id = generate_post_id();
    let root: ResourcePath = format!("/pub/social/v1/posts/{root_id}/{root_id}.json").parse()?;
    test.put(&key, &root, &json!({"kind":"note","content":"root"}))
        .await?;
    let root_key = [author.as_str(), root_id.as_str()];
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    test.put(&key, &path, &json!({"kind":"note","content":"reply with an embed","parent":format!("pubky://{author}/pub/social/v1/posts/{root_id}"),"embed":format!("https://example.com/embedding-reply/{author}")})).await?;
    assert_eq!(
        super::posts::utils::check_member_total_engagement_user_posts(&root_key).await?,
        Some(1),
        "parent engagement = 1 after reply creation"
    );
    let child_id = generate_post_id();
    let child: ResourcePath = format!("/pub/social/v1/posts/{child_id}/{child_id}.json").parse()?;
    test.put(&key, &child, &json!({"kind":"note","content":"keep the reply tombstone","parent":format!("pubky://{author}/pub/social/v1/posts/{id}")})).await?;

    test.del(&key, &path).await?;
    assert!(PostDetails::get_by_id(&author, &id).await?.unwrap().deleted);
    assert!(
        PostRelationships::get_from_index(&author, &id)
            .await?
            .is_some(),
        "the tombstone's cached relationships are its hard delete's gate"
    );

    test.del(&key, &child).await?;
    retrieve_and_handle_event_line(
        &format!("DEL pubky://{author}{path}"),
        test.event_processor_runner.event_handler.clone(),
    )
    .await?;
    assert!(PostDetails::get_from_graph(&author, &id).await?.is_none());
    assert_eq!(
        super::posts::utils::check_member_total_engagement_user_posts(&root_key).await?,
        Some(0),
        "the hard delete takes the reply's engagement off its parent"
    );
    test.del(&key, &root).await?;
    test.del(&key, &profile).await?;
    Ok(())
}

/// Only external targets are Resources. A reply, an embed or a collection item naming a
/// bookmark, a private path or another object Nexus never references is indexed, but makes no
/// Resource, no reference edge, no feed line and no count.
#[tokio_shared_rt::test(shared)]
async fn bookmarks_and_private_paths_never_become_resources() -> Result<()> {
    use super::posts::utils::test_user;
    use pubky_social_specs::legacy_v0::{PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind};

    async fn authored_since(authors: &[&str], mut cursor: u64) -> Result<Vec<ResourceChange>> {
        let mut lines = Vec::new();
        loop {
            let (page, _, _) = ResourceChange::get_from_index(cursor, 1000).await?;
            if page.lines.is_empty() {
                return Ok(lines);
            }
            cursor = page.next_cursor;
            lines.extend(
                page.lines
                    .into_iter()
                    .filter(|line| authors.contains(&line.author.as_str())),
            );
        }
    }

    let mut test = WatcherTest::setup(None).await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;

    // A legacy author: a v0 post takes any URL as its parent or embed
    let legacy = Keypair::random();
    let legacy_id = test
        .create_user(&legacy, &test_user("Private targets", "v0"))
        .await?;
    let legacy_targets = [
        format!("pubky://{legacy_id}/pub/pubky.app/bookmarks/8Z8CWH8NVYQY39ZEBFGKQWWEKG"),
        format!("pubky://{legacy_id}/priv/social/v1/bookmarks/aHR0cHM6Ly9leGFtcGxlLmNvbS8.json"),
        format!("pubky://{legacy_id}/priv/notes.app/drafts/0033SSE3B1FQ0"),
    ];
    let mut legacy_posts = Vec::new();
    for target in &legacy_targets {
        for (parent, embed) in [(Some(target.clone()), None), (None, Some(target.clone()))] {
            let post = PubkyAppPost {
                content: format!("references {target}"),
                kind: PubkyAppPostKind::Short,
                parent,
                embed: embed.map(|uri| PubkyAppPostEmbed {
                    kind: PubkyAppPostKind::Short,
                    uri,
                }),
                attachments: None,
                lock: None,
            };
            legacy_posts.push(test.create_post(&legacy, &post).await?);
        }
    }

    // A social v1 author: a public post references public objects only, so a legacy bookmark
    let social = Keypair::random();
    test.register_user(&social).await?;
    let social_id = social.public_key().to_z32();
    let profile = test
        .put_v1_profile(&social, &v1_test_user("Bookmark targets", ""))
        .await?;
    super::utils::watcher::rank_user(&social_id).await?;
    let bookmark =
        format!("pubky://{social_id}/pub/pubky.app/bookmarks/8Z8CWH8NVYQY39ZEBFGKQWWEKG");
    let mut social_posts = Vec::new();
    for body in [
        json!({"kind":"note","content":"reply to a bookmark","parent":bookmark}),
        json!({"kind":"note","content":"embed of a bookmark","embed":bookmark}),
        json!({"kind":"collection","content":json!({"name":"Bookmarks","items":[{"uri":bookmark}]}).to_string()}),
    ] {
        let id = generate_post_id();
        let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
        test.put(&social, &path, &body).await?;
        social_posts.push((id, path));
    }

    for (id, _) in &legacy_posts {
        assert!(
            PostDetails::get_from_graph(&legacy_id, id).await?.is_some(),
            "{id} must be indexed"
        );
    }
    for (id, _) in &social_posts {
        assert!(
            PostDetails::get_from_graph(&social_id, id).await?.is_some(),
            "{id} must be indexed"
        );
    }
    // A post whose parent makes no Resource is a root post everywhere: in the engagement
    // ranking too, as a graph rebuild puts it
    let posts = legacy_posts
        .iter()
        .map(|(id, _)| (&legacy_id, id))
        .chain(social_posts.iter().map(|(id, _)| (&social_id, id)));
    for (author, id) in posts {
        assert!(
            super::posts::utils::check_member_total_engagement_user_posts(&[author, id])
                .await?
                .is_some(),
            "{id} must be ranked as a root post"
        );
    }
    for target in legacy_targets.iter().chain([&bookmark]) {
        let (uri, _) = normalize_uri(target).map_err(anyhow::Error::msg)?;
        assert!(
            ResourceView::get_by_id(&resource_id(&uri), None, None, None, None)
                .await?
                .is_none(),
            "{target} must not be a Resource"
        );
    }
    let lines = authored_since(&[&legacy_id, &social_id], start).await?;
    assert!(lines.is_empty(), "no reference reaches the feed: {lines:?}");

    for (_, path) in &social_posts {
        test.del(&social, path).await?;
    }
    test.del(&social, &profile).await?;
    for (_, path) in &legacy_posts {
        test.cleanup_post(&legacy, path).await?;
    }
    test.cleanup_user(&legacy).await?;
    Ok(())
}
