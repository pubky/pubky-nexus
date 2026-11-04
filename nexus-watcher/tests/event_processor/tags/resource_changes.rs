use crate::event_processor::posts::utils::{short_post, test_user};
use crate::event_processor::utils::watcher::{retrieve_and_handle_event_line, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::db::OperationOutcome;
use nexus_common::models::event::resource_changes::{
    ResourceChange, ResourceChangeKind, ResourceChangeOp,
};
use nexus_common::models::resource::tag::TagResource;
use nexus_common::universal_tag::normalize::{normalize_uri, resource_id};
use pubky::{Keypair, ResourcePath};
use pubky_social_specs::legacy_v0::{
    post_uri_builder, traits::HashId, user_uri_builder, PubkyAppTag,
};
use std::collections::HashMap;

async fn authored_since(author: &str, mut cursor: u64) -> Result<Vec<ResourceChange>> {
    let mut lines = Vec::new();
    loop {
        let (page, _, _) = ResourceChange::get_from_index(cursor, 1000).await?;
        if page.lines.is_empty() {
            break;
        }
        cursor = page.next_cursor;
        lines.extend(page.lines.into_iter().filter(|line| line.author == author));
    }
    Ok(lines)
}

#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_put_del_and_replay() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Reference feed", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let handler = test.event_processor_runner.event_handler.clone();
    for target in [
        format!("HTTPS://Example.COM:443/feed/{author}#fragment"),
        format!("pubky://{author}/pub/another.app/events/one"),
    ] {
        // Use the specs constructor so the file ID covers the sanitized object.
        let tag = PubkyAppTag::new(target.clone(), "feed".into());
        let path = format!("/pub/pubky.app/tags/{}", tag.create_id());
        let storage: ResourcePath = path.parse()?;
        let tag_uri = format!("pubky://{author}{path}");
        let before = authored_since(&author, start).await?.len();
        test.put(&key, &storage, &tag).await?;
        retrieve_and_handle_event_line(&format!("PUT {tag_uri}"), handler.clone()).await?;
        let lines = authored_since(&author, start).await?;
        assert_eq!(
            lines.len(),
            before + 2,
            "documented at-least-once put delivery"
        );
        assert_eq!(
            lines[before],
            lines[before + 1],
            "replay retains the graph's indexed_at"
        );
        let put = &lines[before];
        assert_eq!(put.kind, ResourceChangeKind::Tag);
        assert_eq!(put.op, ResourceChangeOp::Put);
        assert_eq!(put.object_path, path);
        assert_eq!(put.label.as_deref(), Some("feed"));
        let (normalized, _) = normalize_uri(&target).unwrap();
        assert_eq!(put.uri, normalized);
        assert_eq!(put.resource_id, resource_id(&normalized));
        test.del(&key, &storage).await?;
        retrieve_and_handle_event_line(&format!("DEL {tag_uri}"), handler.clone()).await?;
        let lines = authored_since(&author, start).await?;
        assert_eq!(
            lines.len(),
            before + 3,
            "delete acknowledgement/replay is idempotent"
        );
        let deleted = lines.last().unwrap();
        assert_eq!(deleted.op, ResourceChangeOp::Del);
        assert_eq!(deleted.resource_id, put.resource_id);
        assert_eq!(deleted.object_path, put.object_path);
        assert!(deleted.at >= put.at);
    }
    test.cleanup_user(&key).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_recovers_put_after_graph_commit() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Interrupted reference", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    test = test.remove_event_processing().await;
    let tag = PubkyAppTag::new(
        format!("https://example.com/interrupted/{author}"),
        "feed".into(),
    );
    let tag_id = tag.create_id();
    let path = format!("/pub/pubky.app/tags/{tag_id}");
    let storage: ResourcePath = path.parse()?;
    let tag_uri = format!("pubky://{author}{path}");
    test.put(&key, &storage, &tag).await?;
    let (uri, scheme) = normalize_uri(&tag.uri).unwrap();
    let resource_id = resource_id(&uri);
    let indexed_at = Utc::now().timestamp_millis() - 10_000;

    // Persist exactly the state left by a crash after the graph commit but before
    // ResourceChange::store. Do not publish a feed line on this first attempt.
    let outcome = TagResource::put_to_graph_resource(
        &author,
        &resource_id,
        &uri,
        &scheme,
        &tag_id,
        &tag_uri,
        &tag.label,
        indexed_at,
    )
    .await?;
    assert!(matches!(outcome, OperationOutcome::CreatedOrDeleted));
    assert!(authored_since(&author, start).await?.is_empty());

    // Retry through the real handler: the existing edge must still emit its put.
    let handler = test.event_processor_runner.event_handler.clone();
    let event = format!("PUT {tag_uri}");
    retrieve_and_handle_event_line(&event, handler.clone()).await?;
    let recovered = authored_since(&author, start).await?;
    assert_eq!(
        recovered.len(),
        1,
        "the interrupted append must not be lost"
    );
    assert_eq!(recovered[0].resource_id, resource_id);
    assert_eq!(recovered[0].uri, uri);
    assert_eq!(recovered[0].object_path, path);
    assert_eq!(recovered[0].at, indexed_at);

    // A further retry is a duplicate put, harmless to a consumer keyed by path.
    retrieve_and_handle_event_line(&event, handler.clone()).await?;
    let lines = authored_since(&author, start).await?;
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], lines[1]);
    assert_eq!(lines[0], recovered[0]);
    let mut references = HashMap::new();
    for line in lines {
        assert_eq!(line.op, ResourceChangeOp::Put);
        references.insert((line.author.clone(), line.object_path.clone()), line);
    }
    assert_eq!(references.len(), 1);
    assert_eq!(references.get(&(author.clone(), path)), Some(&recovered[0]));

    test.del(&key, &storage).await?;
    retrieve_and_handle_event_line(&format!("DEL {tag_uri}"), handler.clone()).await?;
    test.cleanup_user(&key).await?;
    retrieve_and_handle_event_line(&format!("DEL {}", user_uri_builder(author)), handler).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_excludes_posts_and_users() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Known references", "feed"))
        .await?;
    let (post_id, post_path) = test
        .create_post(&key, &short_post("Known reference"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    for target in [
        post_uri_builder(author.clone(), post_id),
        user_uri_builder(author.clone()),
    ] {
        let tag = PubkyAppTag {
            uri: target,
            label: "known".into(),
            created_at: Utc::now().timestamp_millis(),
        };
        let path: ResourcePath = format!("/pub/pubky.app/tags/{}", tag.create_id()).parse()?;
        test.put(&key, &path, &tag).await?;
        test.del(&key, &path).await?;
    }
    assert!(authored_since(&author, start).await?.is_empty());
    test.cleanup_post(&key, &post_path).await?;
    test.cleanup_user(&key).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_equivalent_tag_files_share_one_reference() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Equivalent tags", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let handler = test.event_processor_runner.event_handler.clone();
    // Distinct file IDs, one normalized target: both land on one TAGGED edge.
    let files: Vec<(String, ResourcePath, String)> = ["a", "b"]
        .iter()
        .map(|fragment| {
            let tag = PubkyAppTag::new(
                format!("https://example.com/equivalent/{author}#{fragment}"),
                "feed".into(),
            );
            let path = format!("/pub/pubky.app/tags/{}", tag.create_id());
            (
                path.clone(),
                path.parse().unwrap(),
                format!("pubky://{author}{path}"),
            )
        })
        .collect();
    assert_ne!(files[0].0, files[1].0);
    for (fragment, (_, storage, tag_uri)) in ["a", "b"].iter().zip(&files) {
        let tag = PubkyAppTag::new(
            format!("https://example.com/equivalent/{author}#{fragment}"),
            "feed".into(),
        );
        test.put(&key, storage, &tag).await?;
        retrieve_and_handle_event_line(&format!("PUT {tag_uri}"), handler.clone()).await?;
    }
    for (_, storage, tag_uri) in &files {
        test.del(&key, storage).await?;
        retrieve_and_handle_event_line(&format!("DEL {tag_uri}"), handler.clone()).await?;
    }
    let lines = authored_since(&author, start).await?;
    assert!(
        lines.iter().all(|line| line.object_path == files[0].0),
        "only the edge's own file is published: {lines:?}"
    );
    // A consumer applying upsert/delete by key ends with no live reference.
    let mut references = HashMap::new();
    for line in &lines {
        let key = (
            line.author.clone(),
            line.object_path.clone(),
            line.resource_id.clone(),
        );
        match line.op {
            ResourceChangeOp::Put => {
                references.insert(key, ());
            }
            ResourceChangeOp::Del => {
                references.remove(&key);
            }
        }
    }
    assert!(references.is_empty(), "{references:?}");
    assert_eq!(
        lines.last().map(|line| &line.op),
        Some(&ResourceChangeOp::Del)
    );
    test.cleanup_user(&key).await?;
    Ok(())
}

/// A tag's `pubky.app` file and its `social/v1` copy share one edge (one tag id). The copy is
/// gated until the profile flip, which moves the edge to the copy's address: the feed then
/// publishes a put at the v1 path followed by a del at the v0 path, and deleting the v1 file
/// retracts the v1 reference. A consumer keyed by path ends with nothing live.
#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_a_tag_and_its_v1_copy_share_one_reference() -> Result<()> {
    use pubky_social_specs::traits::{HasIdPath as _, HashId as _};
    use pubky_social_specs::PubkySocialTag;

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Tag copies", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let handler = test.event_processor_runner.event_handler.clone();
    let target = format!("https://example.com/copied/{author}");

    let v0 = PubkyAppTag::new(target.clone(), "feed".into());
    let v0_path = format!("/pub/pubky.app/tags/{}", v0.create_id());
    let v1 = PubkySocialTag::new(target.clone(), "feed".into());
    let v1_path = PubkySocialTag::create_path(&v1.create_id());
    let v0_storage: ResourcePath = v0_path.parse()?;
    let v1_storage: ResourcePath = v1_path.parse()?;
    let v0_uri = format!("pubky://{author}{v0_path}");
    let v1_uri = format!("pubky://{author}{v1_path}");

    test.put(&key, &v0_storage, &v0).await?;
    retrieve_and_handle_event_line(&format!("PUT {v0_uri}"), handler.clone()).await?;
    test.put(&key, &v1_storage, &v1).await?;
    retrieve_and_handle_event_line(&format!("PUT {v1_uri}"), handler.clone()).await?;
    // N3 ignores the migration copy until its profile is written last.
    assert!(authored_since(&author, start)
        .await?
        .iter()
        .all(|line| line.object_path == v0_path));
    let before_flip = authored_since(&author, start).await?;
    let profile: ResourcePath = "/pub/social/v1/profile.json".parse()?;
    test.put(&key, &profile, &serde_json::json!({"name":"Tag copies"}))
        .await?;
    let moved = authored_since(&author, start).await?;
    assert_eq!(moved.len(), before_flip.len() + 2, "{moved:?}");
    let (put, del) = (&moved[before_flip.len()], &moved[before_flip.len() + 1]);
    assert_eq!(
        (put.op, put.object_path.as_str()),
        (ResourceChangeOp::Put, v1_path.as_str())
    );
    assert_eq!(put.at, before_flip[0].at, "the move keeps the indexed time");
    assert_eq!(
        (del.op, del.object_path.as_str()),
        (ResourceChangeOp::Del, v0_path.as_str())
    );
    assert_eq!(del.resource_id, put.resource_id);
    // Handled at v1 now: an upsert of the moved reference, published under its v1 path
    retrieve_and_handle_event_line(&format!("PUT {v1_uri}"), handler.clone()).await?;
    test.del(&key, &v1_storage).await?;
    retrieve_and_handle_event_line(&format!("DEL {v1_uri}"), handler.clone()).await?;
    // Gated at v1: publishes nothing
    test.del(&key, &v0_storage).await?;
    retrieve_and_handle_event_line(&format!("DEL {v0_uri}"), handler.clone()).await?;

    let lines = authored_since(&author, start).await?;
    assert!(
        lines[moved.len()..]
            .iter()
            .all(|line| line.object_path == v1_path),
        "after the flip only the v1 file is published: {lines:?}"
    );
    let mut live = HashMap::new();
    for line in &lines {
        let key = (line.object_path.clone(), line.resource_id.clone());
        match line.op {
            ResourceChangeOp::Put => {
                live.insert(key, ());
            }
            ResourceChangeOp::Del => {
                live.remove(&key);
            }
        }
    }
    assert!(live.is_empty(), "{live:?}");
    test.del(&key, &profile).await?;
    Ok(())
}

/// A `pubky.app` DEL that crashed after the graph delete but before the feed append, while
/// its user was still `v0`: after the flip its retry is gated, so the DEL of the active v1
/// copy must finish the journal created under the edge's original v0 path. The edge is gone
/// before the flip, so the flip moves nothing.
#[tokio_shared_rt::test(shared)]
async fn migrated_tag_delete_retry_finishes_the_original_file_journal() -> Result<()> {
    use nexus_common::db::{fetch_row_from_graph, queries};
    use pubky_social_specs::traits::{HasIdPath as _, HashId as _};
    use pubky_social_specs::PubkySocialTag;

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Migrated delete retry", "feed"))
        .await?;
    let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
    let target = format!("https://example.com/migrated-delete/{author}");
    let v0 = PubkyAppTag::new(target.clone(), "retry".into());
    let id = v0.create_id();
    let v0_path: ResourcePath = format!("/pub/pubky.app/tags/{id}").parse()?;
    test.put(&key, &v0_path, &v0).await?;
    let v1 = PubkySocialTag::new(target, "retry".into());
    assert_eq!(id, v1.create_id());
    let v1_path: ResourcePath = PubkySocialTag::create_path(&id).parse()?;
    test.put(&key, &v1_path, &v1).await?;

    let puts = authored_since(&author, cursor).await?;
    assert_eq!(puts.len(), 1);
    let mut deleted = puts[0].clone();
    deleted.op = ResourceChangeOp::Del;
    deleted.at += 1;
    deleted.prepare_delete().await?;
    fetch_row_from_graph(queries::del::delete_tag(&author, &id)).await?;
    // Exact durable state left by a failed finish_delete, without disrupting Redis
    // for the other tests: a journal exists, but the source edge no longer does.
    let profile: ResourcePath = "/pub/social/v1/profile.json".parse()?;
    test.put(
        &key,
        &profile,
        &serde_json::json!({"name":"Migrated retry"}),
    )
    .await?;
    assert_eq!(
        authored_since(&author, cursor).await?,
        puts,
        "the flip has no edge to move"
    );
    test.del(&key, &v1_path).await?;
    retrieve_and_handle_event_line(
        &format!("DEL pubky://{author}{v1_path}"),
        test.event_processor_runner.event_handler.clone(),
    )
    .await?;
    assert_eq!(
        authored_since(&author, cursor).await?,
        vec![puts[0].clone(), deleted]
    );
    test.del(&key, &profile).await?;
    Ok(())
}

/// A flip run that moved the edges but stopped before publishing their moves, the state a
/// crash between the tags step's write and the feed append leaves: the held dels survive,
/// and the flip's own run publishes each move as a put at the v1 path followed by the del
/// at the v0 path, once.
#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_publish_moves_held_by_an_interrupted_flip() -> Result<()> {
    use crate::event_processor::posts::utils::pubky_id;
    use nexus_watcher::events::v0_to_v1;
    use pubky_social_specs::traits::{HasIdPath as _, HashId as _};
    use pubky_social_specs::PubkySocialTag;

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Interrupted flip", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let mut files = Vec::new();
    for n in 0..3 {
        let target = format!("https://example.com/interrupted-flip/{n}/{author}");
        let v0 = PubkyAppTag::new(target.clone(), "moved".into());
        let v0_path = format!("/pub/pubky.app/tags/{}", v0.create_id());
        let v0_storage: ResourcePath = v0_path.parse()?;
        test.put(&key, &v0_storage, &v0).await?;
        // The migrator's copy, ignored until the flip
        let v1 = PubkySocialTag::new(target, "moved".into());
        let v1_path = PubkySocialTag::create_path(&v1.create_id());
        let v1_storage: ResourcePath = v1_path.parse()?;
        test.put(&key, &v1_storage, &v1).await?;
        files.push((v0_path, v1_path, v1_storage));
    }
    let puts = authored_since(&author, start).await?;
    assert_eq!(puts.len(), files.len());

    // The tags step of a flip that stops there: edges moved, moves held, nothing published
    let author_id = pubky_id(&author)?;
    let file_map = v0_to_v1::files::FileMap::load(&author_id).await?;
    v0_to_v1::tags::migrate(&author_id, &file_map, 2).await?;
    assert_eq!(authored_since(&author, start).await?, puts);
    let (_, held) = ResourceChange::pending_moves(&author, 0).await?;
    assert_eq!(held.len(), files.len(), "{held:?}");

    let profile: ResourcePath = "/pub/social/v1/profile.json".parse()?;
    test.put(
        &key,
        &profile,
        &serde_json::json!({"name":"Interrupted flip"}),
    )
    .await?;
    let lines = authored_since(&author, start).await?;
    assert_eq!(lines.len(), puts.len() + 2 * files.len(), "{lines:?}");
    for (v0_path, v1_path, _) in &files {
        let put = puts
            .iter()
            .find(|line| &line.object_path == v0_path)
            .expect("the v0 put");
        let at = lines
            .iter()
            .position(|line| &line.object_path == v1_path)
            .expect("a put at the v1 path");
        let (moved, retracted) = (&lines[at], &lines[at + 1]);
        assert_eq!(moved.op, ResourceChangeOp::Put);
        assert_eq!(
            (&moved.resource_id, &moved.label, moved.at),
            (&put.resource_id, &put.label, put.at)
        );
        assert_eq!(retracted.op, ResourceChangeOp::Del);
        assert_eq!(&retracted.object_path, v0_path);
        assert_eq!(retracted.resource_id, put.resource_id);
    }
    let (_, held) = ResourceChange::pending_moves(&author, 0).await?;
    assert!(held.is_empty(), "{held:?}");

    // The v1 DELs retract what the consumer now holds
    for (_, v1_path, v1_storage) in &files {
        test.del(&key, v1_storage).await?;
        retrieve_and_handle_event_line(
            &format!("DEL pubky://{author}{v1_path}"),
            test.event_processor_runner.event_handler.clone(),
        )
        .await?;
    }
    let mut live = HashMap::new();
    for line in authored_since(&author, start).await? {
        let key = (line.object_path.clone(), line.resource_id.clone());
        match line.op {
            ResourceChangeOp::Put => {
                live.insert(key, ());
            }
            ResourceChangeOp::Del => {
                live.remove(&key);
            }
        }
    }
    assert!(live.is_empty(), "{live:?}");
    test.del(&key, &profile).await?;
    Ok(())
}

/// A flip whose tags step committed but whose later step failed leaves the user at `v0`, the
/// edge at its `social/v1` address and the del of its `pubky.app` address held, until the
/// retry. A `pubky.app` DEL of the tag in that window is handled at `v0` and deletes the edge.
/// The consumer holds only the `pubky.app` path, so once the retried flip has run, nothing of
/// the tag may stay live and nothing may stay held.
#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_retract_a_moved_tag_deleted_before_the_flip_retry() -> Result<()> {
    use crate::event_processor::posts::utils::pubky_id;
    use nexus_watcher::events::v0_to_v1;
    use pubky_social_specs::traits::{HasIdPath as _, HashId as _};
    use pubky_social_specs::PubkySocialTag;

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Delete before flip retry", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let target = format!("https://example.com/delete-before-flip-retry/{author}");
    let v0 = PubkyAppTag::new(target.clone(), "moved".into());
    let v0_path = format!("/pub/pubky.app/tags/{}", v0.create_id());
    let v0_storage: ResourcePath = v0_path.parse()?;
    test.put(&key, &v0_storage, &v0).await?;
    // The migrator's copy, ignored until the flip
    let v1 = PubkySocialTag::new(target, "moved".into());
    let v1_path = PubkySocialTag::create_path(&v1.create_id());
    let v1_storage: ResourcePath = v1_path.parse()?;
    test.put(&key, &v1_storage, &v1).await?;
    let puts = authored_since(&author, start).await?;
    assert_eq!(puts.len(), 1);
    assert_eq!(puts[0].object_path, v0_path);

    // The flip's tags step commits; the step after it fails, so the user stays `v0`
    let author_id = pubky_id(&author)?;
    let file_map = v0_to_v1::files::FileMap::load(&author_id).await?;
    v0_to_v1::tags::migrate(&author_id, &file_map, 2).await?;
    let (_, held) = ResourceChange::pending_moves(&author, 0).await?;
    assert_eq!(held, std::slice::from_ref(&v1_path));

    // Before the retry, the user deletes the tag at its `pubky.app` address
    test.del(&key, &v0_storage).await?;
    retrieve_and_handle_event_line(
        &format!("DEL pubky://{author}{v0_path}"),
        test.event_processor_runner.event_handler.clone(),
    )
    .await?;

    // The retried flip
    let profile: ResourcePath = "/pub/social/v1/profile.json".parse()?;
    test.put(
        &key,
        &profile,
        &serde_json::json!({"name":"Delete before flip retry"}),
    )
    .await?;

    let lines = authored_since(&author, start).await?;
    let mut live = HashMap::new();
    for line in &lines {
        let key = (line.object_path.clone(), line.resource_id.clone());
        match line.op {
            ResourceChangeOp::Put => {
                live.insert(key, ());
            }
            ResourceChangeOp::Del => {
                live.remove(&key);
            }
        }
    }
    assert!(
        live.is_empty(),
        "a deleted tag stays live: {live:?} from {lines:?}"
    );
    let (_, held) = ResourceChange::pending_moves(&author, 0).await?;
    assert!(held.is_empty(), "{held:?}");
    test.del(&key, &v1_storage).await?;
    test.del(&key, &profile).await?;
    Ok(())
}
