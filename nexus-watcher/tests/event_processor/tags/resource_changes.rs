use crate::event_processor::posts::utils::{short_post, test_user};
use crate::event_processor::utils::watcher::{retrieve_and_handle_event_line, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use futures::future::LocalBoxFuture;
use nexus_common::db::OperationOutcome;
use nexus_common::models::event::resource_changes::{
    ResourceChange, ResourceChangeKind, ResourceChangeOp,
};
use nexus_common::models::resource::tag::TagResource;
use nexus_common::universal_tag::normalize::{normalize_uri, resource_id};
use nexus_watcher::EventProcessorError;
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

/// The moves recorded on `author`'s `Resource` tag edges that the feed has not published yet,
/// as `(tag_uri, v0_uri)`: where each edge is, and where it moved from.
pub(super) async fn recorded_moves(author: &str) -> Result<Vec<(String, String)>> {
    use nexus_common::db::{fetch_all_rows_from_graph, graph::Query};

    let query = Query::new(
        "test_recorded_moves",
        "MATCH (:User {id: $author})-[t:TAGGED]->(:Resource)
         WHERE t.moved_at IS NOT NULL
         RETURN t.uri AS tag_uri, t.v0_uri AS v0_uri
         ORDER BY tag_uri",
    )
    .param("author", author);
    let mut moves = Vec::new();
    for row in fetch_all_rows_from_graph(query).await? {
        moves.push((row.get("tag_uri")?, row.get("v0_uri")?));
    }
    Ok(moves)
}

/// The live references a consumer applying `lines` in order holds, by `(object_path,
/// resource_id)`.
fn live_references(lines: &[ResourceChange]) -> std::collections::BTreeSet<(String, String)> {
    let mut live = std::collections::BTreeSet::new();
    for line in lines {
        let key = (line.object_path.clone(), line.resource_id.clone());
        match line.op {
            ResourceChangeOp::Put => {
                live.insert(key);
            }
            ResourceChangeOp::Del => {
                live.remove(&key);
            }
        }
    }
    live
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
    deleted.prepare_delete(None).await?;
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

/// A flip whose tags step moved the edges and then stopped, before the epoch write: the moves
/// stay recorded on the edges, and the retried flip publishes each, after its epoch write, as
/// a put at the v1 path followed by the del at the v0 path, once.
#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_publish_moves_recorded_by_an_interrupted_flip() -> Result<()> {
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

    // The tags step of a flip that stops there: edges moved, moves recorded, nothing published
    let author_id = pubky_id(&author)?;
    let file_map = v0_to_v1::files::FileMap::load(&author_id).await?;
    v0_to_v1::tags::migrate(&author_id, &file_map, 2).await?;
    assert_eq!(authored_since(&author, start).await?, puts);
    let recorded = recorded_moves(&author).await?;
    assert_eq!(recorded.len(), files.len(), "{recorded:?}");

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
    let recorded = recorded_moves(&author).await?;
    assert!(recorded.is_empty(), "{recorded:?}");

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
/// edge at its `social/v1` address and its move recorded, until the retry. A `pubky.app` DEL
/// of the tag in that window is handled at `v0` and deletes the edge. The consumer holds only
/// the `pubky.app` path, so the DEL retracts it next to the `social/v1` one, and once the
/// retried flip has run, nothing of the tag may stay live and no move may stay recorded.
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
    let recorded = recorded_moves(&author).await?;
    assert_eq!(
        recorded,
        [(
            format!("pubky://{author}{v1_path}"),
            format!("pubky://{author}{v0_path}")
        )]
    );

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
    let recorded = recorded_moves(&author).await?;
    assert!(recorded.is_empty(), "{recorded:?}");
    test.del(&key, &v1_storage).await?;
    test.del(&key, &profile).await?;
    Ok(())
}

/// A resource tag `author` wrote on `target` at `pubky.app`, and the migrator's `social/v1`
/// copy of it, ignored until the flip; then the flip, whose publish of the move fails. Returns
/// the paths of both files, the copy's storage and the profile.
struct UnpublishedMove {
    v0_path: String,
    v1_path: String,
    v1_storage: ResourcePath,
    profile: ResourcePath,
    puts: Vec<ResourceChange>,
}

async fn flip_failing_to_publish_a_move(
    test: &mut WatcherTest,
    key: &Keypair,
    author: &str,
    target: &str,
    start: u64,
) -> Result<UnpublishedMove> {
    use nexus_common::models::user::Epoch;
    use nexus_common::utils::test_utils::fail_move_appends;
    use pubky_social_specs::traits::{HasIdPath as _, HashId as _};
    use pubky_social_specs::PubkySocialTag;

    let v0 = PubkyAppTag::new(target.to_string(), "moved".into());
    let v0_path = format!("/pub/pubky.app/tags/{}", v0.create_id());
    test.put(key, &v0_path.parse()?, &v0).await?;
    let v1 = PubkySocialTag::new(target.to_string(), "moved".into());
    let v1_path = PubkySocialTag::create_path(&v1.create_id());
    let v1_storage: ResourcePath = v1_path.parse()?;
    test.put(key, &v1_storage, &v1).await?;
    let puts = authored_since(author, start).await?;
    assert_eq!(puts.len(), 1);

    fail_move_appends(author, true);
    let profile: ResourcePath = "/pub/social/v1/profile.json".parse()?;
    let flipped = test
        .put(
            key,
            &profile,
            &serde_json::json!({"name": "Unpublished move"}),
        )
        .await;
    fail_move_appends(author, false);
    flipped?;
    assert_eq!(Epoch::get_from_graph(author).await?, Epoch::V1);
    assert_eq!(
        authored_since(author, start).await?,
        puts,
        "the failed publish appends nothing"
    );
    assert_eq!(
        recorded_moves(author).await?,
        [(
            format!("pubky://{author}{v1_path}"),
            format!("pubky://{author}{v0_path}")
        )],
        "the move stays recorded"
    );
    Ok(UnpublishedMove {
        v0_path,
        v1_path,
        v1_storage,
        profile,
        puts,
    })
}

/// The feed never fails the flip: when the append of a move fails, the user is `v1` all the
/// same, the move stays recorded on the edge, and the sweep publishes it as a put at the v1
/// path followed by a del at the v0 path, once.
#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_flip_succeeds_when_its_publish_fails() -> Result<()> {
    use nexus_watcher::events::v0_to_v1::resource_changes;

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Publish fails", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let target = format!("https://example.com/publish-fails/{author}");
    let moved = flip_failing_to_publish_a_move(&mut test, &key, &author, &target, start).await?;

    resource_changes::sweep(resource_changes::BATCH_SIZE).await?;
    let lines = authored_since(&author, start).await?;
    let put = &moved.puts[0];
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert_eq!(
        (lines[1].op, lines[1].object_path.as_str(), lines[1].at),
        (ResourceChangeOp::Put, moved.v1_path.as_str(), put.at)
    );
    assert_eq!(
        (lines[2].op, lines[2].object_path.as_str()),
        (ResourceChangeOp::Del, moved.v0_path.as_str())
    );
    assert_eq!(
        (&lines[1].resource_id, &lines[2].resource_id),
        (&put.resource_id, &put.resource_id)
    );
    assert!(recorded_moves(&author).await?.is_empty());

    // Published once
    resource_changes::sweep(resource_changes::BATCH_SIZE).await?;
    assert_eq!(authored_since(&author, start).await?, lines);
    test.del(&key, &moved.v1_storage).await?;
    test.del(&key, &moved.profile).await?;
    Ok(())
}

/// The tags step records the address an edge moved from once: a rerun of its write over an
/// edge already at its new address, as a retried flip's would be, keeps the recorded address
/// and time, and the move is published from the original address.
#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_tags_step_rerun_keeps_the_original_v0_uri() -> Result<()> {
    use crate::event_processor::posts::utils::pubky_id;
    use nexus_common::db::{
        fetch_all_rows_from_graph, fetch_key_from_graph, graph::Query, queries,
    };
    use nexus_watcher::events::v0_to_v1;
    use pubky_social_specs::traits::{HasIdPath as _, HashId as _};
    use pubky_social_specs::PubkySocialTag;

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Tags step rerun", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let target = format!("https://example.com/tags-step-rerun/{author}");
    let v0 = PubkyAppTag::new(target.clone(), "moved".into());
    let v0_path = format!("/pub/pubky.app/tags/{}", v0.create_id());
    test.put(&key, &v0_path.parse()?, &v0).await?;
    let v1 = PubkySocialTag::new(target, "moved".into());
    let v1_path = PubkySocialTag::create_path(&v1.create_id());
    let v1_uri = format!("pubky://{author}{v1_path}");
    let v0_uri = format!("pubky://{author}{v0_path}");

    let author_id = pubky_id(&author)?;
    let files = v0_to_v1::files::FileMap::load(&author_id).await?;
    v0_to_v1::tags::migrate(&author_id, &files, 2).await?;
    let moved = || async {
        let query = Query::new(
            "test_moved_edge",
            "MATCH (:User {id: $author})-[t:TAGGED]->(:Resource)
             RETURN elementId(t) AS edge, t.id AS id, t.uri AS uri, t.v0_uri AS v0_uri,
                 t.moved_at AS moved_at",
        )
        .param("author", author.as_str());
        let rows = fetch_all_rows_from_graph(query).await?;
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        anyhow::Ok((
            row.get::<String>("edge")?,
            row.get::<String>("id")?,
            row.get::<String>("uri")?,
            row.get::<String>("v0_uri")?,
            row.get::<i64>("moved_at")?,
        ))
    };
    let (edge, id, uri, recorded_from, moved_at) = moved().await?;
    assert_eq!(
        (uri.as_str(), recorded_from.as_str()),
        (v1_uri.as_str(), v0_uri.as_str())
    );

    // The write again, later, over the moved edge
    let changed = fetch_key_from_graph::<i64>(
        queries::migration::set_tag_addresses(
            &author,
            &[(edge.clone(), id.clone(), v1_uri.clone())],
            moved_at + 60_000,
        ),
        "changed",
    )
    .await?;
    assert_eq!(changed, Some(1));
    // And the whole step: no edge is left under `pubky.app`
    v0_to_v1::tags::migrate(&author_id, &files, 2).await?;
    assert_eq!(moved().await?, (edge, id, v1_uri, v0_uri, moved_at));

    v0_to_v1::resource_changes::publish_moves(&author_id).await?;
    let lines = authored_since(&author, start).await?;
    assert_eq!(
        lines
            .iter()
            .map(|line| (line.op, line.object_path.as_str()))
            .collect::<Vec<_>>(),
        [
            (ResourceChangeOp::Put, v0_path.as_str()),
            (ResourceChangeOp::Put, v1_path.as_str()),
            (ResourceChangeOp::Del, v0_path.as_str())
        ]
    );
    test.del(&key, &v0_path.parse()?).await?;
    test.cleanup_user(&key).await?;
    Ok(())
}

/// A v1 DEL of a moved tag whose move the flip failed to publish: the consumer holds the tag
/// only at its `pubky.app` path, so the DEL retracts that path next to its own, and the sweep
/// then has nothing left to publish for it.
#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_delete_of_an_unpublished_move_retracts_the_old_path() -> Result<()> {
    use nexus_watcher::events::v0_to_v1::resource_changes;

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Delete unpublished move", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let target = format!("https://example.com/delete-unpublished-move/{author}");
    let moved = flip_failing_to_publish_a_move(&mut test, &key, &author, &target, start).await?;

    test.del(&key, &moved.v1_storage).await?;
    retrieve_and_handle_event_line(
        &format!("DEL pubky://{author}{}", moved.v1_path),
        test.event_processor_runner.event_handler.clone(),
    )
    .await?;
    let lines = authored_since(&author, start).await?;
    assert_eq!(
        lines[1..]
            .iter()
            .map(|line| (line.op, line.object_path.as_str()))
            .collect::<Vec<_>>(),
        [
            (ResourceChangeOp::Del, moved.v1_path.as_str()),
            (ResourceChangeOp::Del, moved.v0_path.as_str())
        ],
        "{lines:?}"
    );
    assert!(live_references(&lines).is_empty(), "{lines:?}");
    assert!(recorded_moves(&author).await?.is_empty());

    resource_changes::sweep(resource_changes::BATCH_SIZE).await?;
    assert_eq!(authored_since(&author, start).await?, lines);
    test.del(&key, &moved.profile).await?;
    Ok(())
}

/// A v1 DEL of a tag whose move is not published journals the del of the old path beside its
/// own, and fails to publish after its graph delete. Before its retry, the user tags the same
/// target again, at the same v1 path: that put supersedes the journal of its path, and
/// publishes the del of the old path pending with it, which the put does not bring back.
#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_put_again_after_a_failed_delete_retracts_the_old_path() -> Result<()>
{
    use deadpool_redis::redis::AsyncCommands;
    use nexus_common::db::get_redis_conn;
    use nexus_watcher::events::handlers::tag;
    use nexus_watcher::events::inputs::ResourceKey;
    use pubky_social_specs::traits::HashId as _;
    use pubky_social_specs::PubkySocialTag;

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Put again after failed delete", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let target = format!("https://example.com/put-again-after-failed-delete/{author}");
    let moved = flip_failing_to_publish_a_move(&mut test, &key, &author, &target, start).await?;
    let v1_uri = format!("pubky://{author}{}", moved.v1_path);
    let delete_key = || -> Result<ResourceKey> {
        Ok(ResourceKey {
            owner_id: crate::event_processor::posts::utils::pubky_id(&author)?,
            resource_id: PubkySocialTag::new(target.clone(), "moved".into()).create_id(),
        })
    };

    // The DEL fails after its graph delete: its path's journal key holds another type
    let journal = format!(
        "Event:ResourceChanges:PendingDelete:{author}:{}",
        moved.v1_path
    );
    let mut conn = get_redis_conn().await?;
    let _: () = conn.hset(&journal, "of", "another type").await?;
    assert!(
        tag::del_at(delete_key()?, &v1_uri).await.is_err(),
        "the DEL must fail to publish"
    );
    let _: () = conn.del(&journal).await?;
    assert!(
        recorded_moves(&author).await?.is_empty(),
        "the edge is gone"
    );

    // The same tag again, at the same v1 path
    let v1 = PubkySocialTag::new(target.clone(), "moved".into());
    test.put(&key, &moved.v1_storage, &v1).await?;
    let lines = authored_since(&author, start).await?;
    assert!(
        lines[1..]
            .iter()
            .any(|line| line.op == ResourceChangeOp::Del && line.object_path == moved.v0_path),
        "the old path must be retracted: {lines:?}"
    );
    assert_eq!(
        live_references(&lines),
        [(moved.v1_path.clone(), moved.puts[0].resource_id.clone())].into(),
        "{lines:?}"
    );
    test.del(&key, &moved.v1_storage).await?;
    test.del(&key, &moved.profile).await?;
    Ok(())
}

/// The sweep and a DEL of a moved tag, queued for the tag file's lock in either order: the
/// DEL either runs first, and the sweep finds no edge, or after the sweep published the move,
/// and retracts it. No put of the tag outlives the DEL.
#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_sweep_and_delete_agree_in_either_order() -> Result<()> {
    use futures::FutureExt;
    use nexus_watcher::events::handlers::tag;
    use nexus_watcher::events::inputs::ResourceKey;
    use nexus_watcher::events::v0_to_v1::resource_changes;

    let mut test = WatcherTest::setup(None).await?;
    for sweep_first in [true, false] {
        let key = Keypair::random();
        let author = test
            .create_user(&key, &test_user("Sweep and delete", "feed"))
            .await?;
        let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
        let target = format!("https://example.com/sweep-and-delete/{author}");
        let moved =
            flip_failing_to_publish_a_move(&mut test, &key, &author, &target, start).await?;
        let id = moved
            .v1_path
            .rsplit('/')
            .next()
            .unwrap()
            .trim_end_matches(".json")
            .to_string();
        let v1_uri = format!("pubky://{author}{}", moved.v1_path);
        let delete_key = ResourceKey {
            owner_id: crate::event_processor::posts::utils::pubky_id(&author)?,
            resource_id: id.clone(),
        };

        let guard = tag::lock_tag(&author, &id).await;
        let mut sweep = async {
            resource_changes::sweep(resource_changes::BATCH_SIZE)
                .await
                .map(|_| ())
        }
        .boxed_local();
        let mut delete = async move { tag::del_at(delete_key, &v1_uri).await }.boxed_local();
        let order = if sweep_first {
            [&mut sweep, &mut delete]
        } else {
            [&mut delete, &mut sweep]
        };
        for handler in order {
            // The sweep reads its page first, then waits for the lock
            assert!(
                tokio::time::timeout(std::time::Duration::from_secs(2), handler.as_mut())
                    .await
                    .is_err(),
                "sweep_first={sweep_first}: every handler must wait for the tag's lock"
            );
        }
        drop(guard);
        let (swept, deleted) = futures::join!(sweep, delete);
        swept?;
        deleted?;

        let lines = authored_since(&author, start).await?;
        assert!(
            live_references(&lines).is_empty(),
            "sweep_first={sweep_first}: a deleted tag stays live: {lines:?}"
        );
        // The sweep publishes other users' recorded moves too, and may reach this tag's lock
        // only after the DEL: either order is one of these
        let swept_then_deleted = [
            (ResourceChangeOp::Put, moved.v1_path.as_str()),
            (ResourceChangeOp::Del, moved.v0_path.as_str()),
            (ResourceChangeOp::Del, moved.v1_path.as_str()),
        ];
        let deleted_then_swept = [
            (ResourceChangeOp::Del, moved.v1_path.as_str()),
            (ResourceChangeOp::Del, moved.v0_path.as_str()),
        ];
        let after: Vec<_> = lines[1..]
            .iter()
            .map(|line| (line.op, line.object_path.as_str()))
            .collect();
        assert!(
            after == swept_then_deleted || after == deleted_then_swept,
            "sweep_first={sweep_first}: {lines:?}"
        );
        if !sweep_first {
            assert_eq!(after, deleted_then_swept, "the DEL ran first");
        }
        assert!(recorded_moves(&author).await?.is_empty());
        test.del(&key, &moved.v1_storage).await?;
        test.del(&key, &moved.profile).await?;
    }
    Ok(())
}

/// A publish that appended a move and then stopped before clearing its record, as a crash
/// between the two leaves it: the next sweep appends the move again and clears the record.
/// The consumer only sees the put and the del twice, and ends with the reference at its
/// `social/v1` path.
#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_a_move_appended_but_not_cleared_is_only_repeated() -> Result<()> {
    use nexus_watcher::events::v0_to_v1::resource_changes;

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Move not cleared", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let target = format!("https://example.com/move-not-cleared/{author}");
    let moved = flip_failing_to_publish_a_move(&mut test, &key, &author, &target, start).await?;

    // The append of the publish, without its clear
    let put = &moved.puts[0];
    let mut at_v1 = put.clone();
    at_v1.object_path = moved.v1_path.clone();
    let mut del_v0 = put.clone();
    del_v0.op = ResourceChangeOp::Del;
    ResourceChange::store_moves(&[(at_v1, del_v0)]).await?;
    assert_eq!(recorded_moves(&author).await?.len(), 1);

    resource_changes::sweep(resource_changes::BATCH_SIZE).await?;
    let lines = authored_since(&author, start).await?;
    assert_eq!(
        lines[1..]
            .iter()
            .map(|line| (line.op, line.object_path.as_str()))
            .collect::<Vec<_>>(),
        [
            (ResourceChangeOp::Put, moved.v1_path.as_str()),
            (ResourceChangeOp::Del, moved.v0_path.as_str()),
            (ResourceChangeOp::Put, moved.v1_path.as_str()),
            (ResourceChangeOp::Del, moved.v0_path.as_str())
        ],
        "{lines:?}"
    );
    assert_eq!(
        live_references(&lines),
        [(moved.v1_path.clone(), put.resource_id.clone())].into()
    );
    assert!(recorded_moves(&author).await?.is_empty());
    resource_changes::sweep(resource_changes::BATCH_SIZE).await?;
    assert_eq!(authored_since(&author, start).await?, lines);
    test.del(&key, &moved.v1_storage).await?;
    test.del(&key, &moved.profile).await?;
    Ok(())
}

/// A resource tag file's handlers, as the watcher runs them: a PUT through
/// `sync_put_resource`, a DEL through `del_at`.
#[derive(Clone, Copy, Debug)]
enum TagStep {
    Put,
    Del,
}

/// The handler of `step` for the tag file of `author` on `target` labelled `steps`, not yet
/// polled.
fn tag_handler(
    author: &str,
    target: &str,
    step: TagStep,
) -> Result<LocalBoxFuture<'static, Result<(), EventProcessorError>>> {
    use futures::FutureExt;
    use nexus_watcher::events::handlers::tag;
    use nexus_watcher::events::inputs::{Reference, ReferenceTarget, ResourceKey, TagInput};

    let id = PubkyAppTag::new(target.to_string(), "steps".into()).create_id();
    let tag_uri = format!("pubky://{author}/pub/pubky.app/tags/{id}");
    let key = ResourceKey {
        owner_id: crate::event_processor::posts::utils::pubky_id(author)?,
        resource_id: id,
    };
    Ok(match step {
        TagStep::Put => {
            let input = TagInput {
                uri: tag_uri,
                tagged: Reference {
                    uri: target.to_string(),
                    target: ReferenceTarget::Foreign,
                },
                label: "steps".into(),
            };
            tag::sync_put_resource(input, key).boxed_local()
        }
        TagStep::Del => async move { tag::del_at(key, &tag_uri).await }.boxed_local(),
    })
}

/// Polls each of `handlers` once, asserting it waits for its tag's lock: they queue for it in
/// that order.
async fn queue_tag_handlers(
    steps: &[TagStep],
    handlers: &mut [LocalBoxFuture<'static, Result<(), EventProcessorError>>],
) {
    for (step, handler) in steps.iter().zip(handlers.iter_mut()) {
        assert!(
            futures::poll!(handler.as_mut()).is_pending(),
            "{step:?} must wait for the tag's lock"
        );
    }
}

/// Queues `steps` for one tag file behind its lock, in that order, then lets them run: the
/// lock grants them one at a time, first come first served. Each step is polled until it waits
/// for the lock, so the order is fixed before any of them runs. Returns the file's path.
async fn run_tag_steps(author: &str, target: &str, steps: &[TagStep]) -> Result<String> {
    use futures::future::join_all;
    use nexus_watcher::events::handlers::tag;

    let id = PubkyAppTag::new(target.to_string(), "steps".into()).create_id();
    let path = format!("/pub/pubky.app/tags/{id}");
    let guard = tag::lock_tag(author, &id).await;
    let mut queued = steps
        .iter()
        .map(|step| tag_handler(author, target, *step))
        .collect::<Result<Vec<_>>>()?;
    queue_tag_handlers(steps, &mut queued).await;
    drop(guard);
    for (step, result) in steps.iter().zip(join_all(queued).await) {
        result.map_err(|e| anyhow::anyhow!("{step:?}: {e}"))?;
    }
    Ok(path)
}

/// The flip's tags step and its publish step for `author`, as the flip runs them, not yet
/// polled.
fn flip_tags(
    author: &str,
    batch_size: usize,
) -> Result<LocalBoxFuture<'static, Result<(), EventProcessorError>>> {
    use futures::FutureExt;
    use nexus_watcher::events::v0_to_v1;

    let author_id = crate::event_processor::posts::utils::pubky_id(author)?;
    Ok(async move {
        let files = v0_to_v1::files::FileMap::load(&author_id).await?;
        v0_to_v1::tags::migrate(&author_id, &files, batch_size).await?;
        v0_to_v1::resource_changes::publish_moves(&author_id)
            .await
            .map(|_| ())
    }
    .boxed_local())
}

/// The feed's live references of `author` to `target` since `start` are exactly the graph's:
/// the edge's reference when it exists, none otherwise.
async fn assert_feed_matches_graph(author: &str, start: u64, target: &str) -> Result<()> {
    use nexus_common::db::{fetch_row_from_graph, queries};

    let (normalized, _) = normalize_uri(target).unwrap();
    let rid = resource_id(&normalized);
    let edge = fetch_row_from_graph(queries::get::get_resource_tag_indexed_at(
        author, &rid, "steps",
    ))
    .await?;
    let lines: Vec<_> = authored_since(author, start)
        .await?
        .into_iter()
        .filter(|line| line.resource_id == rid)
        .collect();
    let mut live = std::collections::BTreeSet::new();
    for line in &lines {
        let key = (line.object_path.clone(), line.resource_id.clone());
        match line.op {
            ResourceChangeOp::Put => {
                live.insert(key);
            }
            ResourceChangeOp::Del => {
                live.remove(&key);
            }
        }
    }
    let expected: std::collections::BTreeSet<_> = match edge {
        Some(row) => {
            let tag_uri: String = row.get("tag_uri")?;
            let path = tag_uri
                .strip_prefix(&format!("pubky://{author}"))
                .unwrap()
                .to_string();
            [(path, rid)].into()
        }
        None => Default::default(),
    };
    assert_eq!(live, expected, "feed {lines:?}");
    Ok(())
}

/// A DEL of a resource tag waits while another handler of the same tag file holds its lock,
/// and runs once it is released.
#[tokio_shared_rt::test(shared)]
async fn test_resource_tag_delete_waits_for_the_tag_lock() -> Result<()> {
    use nexus_watcher::events::handlers::tag;
    use nexus_watcher::events::inputs::ResourceKey;

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Tag lock", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let target = format!("https://example.com/tag-lock/{author}");
    let tag = PubkyAppTag::new(target.clone(), "steps".into());
    let id = tag.create_id();
    let storage: ResourcePath = format!("/pub/pubky.app/tags/{id}").parse()?;
    let tag_uri = format!("pubky://{author}{storage}");
    test.put(&key, &storage, &tag).await?;

    let guard = tag::lock_tag(&author, &id).await;
    let owner_id = crate::event_processor::posts::utils::pubky_id(&author)?;
    let mut delete = tokio::spawn(async move {
        tag::del_at(
            ResourceKey {
                owner_id,
                resource_id: id,
            },
            &tag_uri,
        )
        .await
    });
    // Bounds the wait only: a DEL that does not take the lock finishes well within it
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(2), &mut delete)
            .await
            .is_err(),
        "a DEL must wait while the tag's lock is held"
    );
    drop(guard);
    delete.await??;
    assert_feed_matches_graph(&author, start, &target).await?;
    let lines = authored_since(&author, start).await?;
    assert_eq!(
        lines.last().map(|line| line.op),
        Some(ResourceChangeOp::Del)
    );
    test.cleanup_user(&key).await?;
    Ok(())
}

/// A PUT and a DEL of the same resource tag file, run concurrently in either order, with or
/// without the edge already there, leave the feed's live references equal to the graph's.
#[tokio_shared_rt::test(shared)]
async fn test_resource_tag_put_and_delete_agree_in_either_order() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Tag orders", "feed"))
        .await?;
    for (n, (existing, steps)) in [
        (false, [TagStep::Put, TagStep::Del]),
        (false, [TagStep::Del, TagStep::Put]),
        (true, [TagStep::Put, TagStep::Del]),
        (true, [TagStep::Del, TagStep::Put]),
    ]
    .into_iter()
    .enumerate()
    {
        let target = format!("https://example.com/tag-orders/{n}/{author}");
        let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
        if existing {
            run_tag_steps(&author, &target, &[TagStep::Put]).await?;
        }
        run_tag_steps(&author, &target, &steps).await?;
        assert_feed_matches_graph(&author, start, &target)
            .await
            .map_err(|e| anyhow::anyhow!("existing={existing} {steps:?}: {e}"))?;
    }
    test.cleanup_user(&key).await?;
    Ok(())
}

/// Two PUTs of the same resource tag file around a DEL of it, all concurrent: the second PUT
/// re-creates the edge after the DEL, so the reference is live, as the edge is.
#[tokio_shared_rt::test(shared)]
async fn test_resource_tag_put_delete_put_leaves_the_reference_live() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Tag three handlers", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let target = format!("https://example.com/tag-three/{author}");
    run_tag_steps(&author, &target, &[TagStep::Put]).await?;
    run_tag_steps(
        &author,
        &target,
        &[TagStep::Put, TagStep::Del, TagStep::Put],
    )
    .await?;
    assert_feed_matches_graph(&author, start, &target).await?;
    let lines = authored_since(&author, start).await?;
    assert_eq!(
        lines.last().map(|line| line.op),
        Some(ResourceChangeOp::Put)
    );
    test.cleanup_user(&key).await?;
    Ok(())
}

/// The flip's tags step moves a resource tag edge only under the tag file's lock, so it never
/// moves the edge between a handler's read of it and that handler's append. Here the test holds
/// the lock as a PUT of the file does once it has read the edge's owner, its own `pubky.app`
/// path: the flip waits, the PUT appends its put of that path, and a PUT and a DEL of the file
/// queue for the lock. Once all have run, the feed holds what the graph holds.
#[tokio_shared_rt::test(shared)]
async fn test_resource_tag_flip_waits_for_the_tag_lock() -> Result<()> {
    use futures::future::join_all;
    use nexus_common::db::{fetch_row_from_graph, queries};
    use nexus_watcher::events::handlers::tag;

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Flip lock", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let target = format!("https://example.com/flip-lock/{author}");
    let path = run_tag_steps(&author, &target, &[TagStep::Put]).await?;
    let id = PubkyAppTag::new(target.clone(), "steps".into()).create_id();

    let guard = tag::lock_tag(&author, &id).await;
    let mut flip = flip_tags(&author, 2)?;
    // Bounds the wait only: a flip that does not take the lock finishes well within it
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(2), &mut flip)
            .await
            .is_err(),
        "the flip must wait while the tag's lock is held"
    );
    // The PUT holding the lock found its own path on the edge, and appends its put
    let (normalized, _) = normalize_uri(&target).unwrap();
    let rid = resource_id(&normalized);
    let row = fetch_row_from_graph(queries::get::get_resource_tag_indexed_at(
        &author, &rid, "steps",
    ))
    .await?
    .expect("the edge");
    ResourceChange::tag(
        ResourceChangeOp::Put,
        rid,
        normalized.to_string(),
        author.clone(),
        "steps".into(),
        &format!("pubky://{author}{path}"),
        row.get("tag_indexed_at")?,
    )?
    .store()
    .await?;
    let steps = [TagStep::Put, TagStep::Del];
    let mut queued = steps
        .iter()
        .map(|step| tag_handler(&author, &target, *step))
        .collect::<Result<Vec<_>>>()?;
    queue_tag_handlers(&steps, &mut queued).await;
    drop(guard);
    let (flipped, handled) = futures::join!(flip, join_all(queued));
    flipped?;
    for (step, result) in steps.iter().zip(handled) {
        result.map_err(|e| anyhow::anyhow!("{step:?}: {e}"))?;
    }

    assert_feed_matches_graph(&author, start, &target).await?;
    let recorded = recorded_moves(&author).await?;
    assert!(recorded.is_empty(), "{recorded:?}");
    test.cleanup_user(&key).await?;
    Ok(())
}

/// The flip's tags step takes the locks of a page's tag files together. Two files whose locks
/// share a stripe take it once: the flip moves both, and publishes both moves.
#[tokio_shared_rt::test(shared)]
async fn test_resource_tag_flip_moves_tags_that_share_a_lock() -> Result<()> {
    use nexus_watcher::events::handlers::tag;

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Flip shared lock", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let mut stripes = HashMap::new();
    let targets = (0..)
        .map(|n| format!("https://example.com/flip-shared-lock/{n}/{author}"))
        .find_map(|target| {
            let id = PubkyAppTag::new(target.clone(), "steps".into()).create_id();
            stripes
                .insert(tag::tag_lock_stripe(&author, &id), target.clone())
                .map(|other| [other, target])
        })
        .unwrap();
    for target in &targets {
        run_tag_steps(&author, target, &[TagStep::Put]).await?;
    }

    // Bounds a deadlock only: the flip of two edges finishes well within it
    tokio::time::timeout(std::time::Duration::from_secs(30), flip_tags(&author, 2)?)
        .await
        .expect("the flip must not wait for itself")?;

    for target in &targets {
        assert_feed_matches_graph(&author, start, target).await?;
    }
    let moved = authored_since(&author, start)
        .await?
        .into_iter()
        .filter(|line| {
            line.op == ResourceChangeOp::Put && line.object_path.starts_with("/pub/social/v1/")
        })
        .count();
    assert_eq!(moved, 2);
    test.cleanup_user(&key).await?;
    Ok(())
}

/// A resource tag `author` wrote, labelled `steps`, on their own `v0` file, and the migrator's
/// `social/v1` copy of it on the file's migrated address, ignored until the flip. The flip
/// gives the edge the copy's id.
struct OwnFileTag {
    file_uri: String,
    file_path: ResourcePath,
    v0_path: String,
    v1_id: String,
    v1_storage: ResourcePath,
}

async fn put_own_file_tag(
    test: &mut WatcherTest,
    key: &Keypair,
    author: &str,
) -> Result<OwnFileTag> {
    use crate::event_processor::v0_to_v1::attachments::{create_file, upload_blob, v1_file_uri};
    use pubky_social_specs::traits::{HasIdPath as _, HashId as _};
    use pubky_social_specs::PubkySocialTag;

    let unique = format!("{author}:{}", Utc::now().timestamp_micros());
    let (hash, src) = upload_blob(test, key, author, &format!("own file tag:{unique}")).await?;
    let (_, file_path, file_uri) =
        create_file(test, key, author, "tagged.png", &src, "image/png").await?;
    let v0 = PubkyAppTag::new(file_uri.clone(), "steps".into());
    let v0_path = format!("/pub/pubky.app/tags/{}", v0.create_id());
    test.put(key, &v0_path.parse()?, &v0).await?;
    let v1 = PubkySocialTag::new(v1_file_uri(author, &hash, "png"), "steps".into());
    let v1_id = v1.create_id();
    assert_ne!(
        v1_id,
        v0.create_id(),
        "the flip gives an own-file tag a new id"
    );
    let v1_storage: ResourcePath = PubkySocialTag::create_path(&v1_id).parse()?;
    test.put(key, &v1_storage, &v1).await?;
    Ok(OwnFileTag {
        file_uri,
        file_path,
        v0_path,
        v1_id,
        v1_storage,
    })
}

/// The address stored on `author`'s `steps` edge to `target`, if the edge exists.
async fn resource_tag_edge(author: &str, target: &str) -> Result<Option<String>> {
    use nexus_common::db::{fetch_row_from_graph, queries};

    let (normalized, _) = normalize_uri(target).unwrap();
    let edge = fetch_row_from_graph(queries::get::get_resource_tag_indexed_at(
        author,
        &resource_id(&normalized),
        "steps",
    ))
    .await?;
    Ok(edge.map(|row| row.get("tag_uri")).transpose()?)
}

/// After a flip whose tags step moved an own-file tag to its new id and address, then failed,
/// `remove` deletes or takes down the tag at its `pubky.app` address while the user is still
/// `v0`. The retried flip must leave no edge, no live reference and no recorded move.
async fn own_file_tag_removed_before_the_flip_retry(
    name: &str,
    remove: impl AsyncFnOnce(&mut WatcherTest, &Keypair, &str, &OwnFileTag) -> Result<()>,
) -> Result<()> {
    own_file_tag_removed_after_a_failed_flip(name, false, remove).await
}

/// [`own_file_tag_removed_before_the_flip_retry`], where the move has been `published` or not
/// before the removal: the sweep publishes the moves of a flip that failed at its epoch write.
async fn own_file_tag_removed_after_a_failed_flip(
    name: &str,
    published: bool,
    remove: impl AsyncFnOnce(&mut WatcherTest, &Keypair, &str, &OwnFileTag) -> Result<()>,
) -> Result<()> {
    use crate::event_processor::posts::utils::pubky_id;
    use nexus_watcher::events::v0_to_v1;

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test.create_user(&key, &test_user(name, "feed")).await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let tag = put_own_file_tag(&mut test, &key, &author).await?;

    // The flip's tags step commits; the step after it fails, so the user stays `v0`
    let author_id = pubky_id(&author)?;
    let file_map = v0_to_v1::files::FileMap::load(&author_id).await?;
    v0_to_v1::tags::migrate(&author_id, &file_map, 2).await?;
    if published {
        v0_to_v1::resource_changes::publish_moves(&author_id).await?;
    }
    assert_eq!(
        resource_tag_edge(&author, &tag.file_uri).await?,
        Some(format!("pubky://{author}{}", tag.v1_storage))
    );

    remove(&mut test, &key, &author, &tag).await?;

    // The retried flip
    let profile: ResourcePath = "/pub/social/v1/profile.json".parse()?;
    test.put(&key, &profile, &serde_json::json!({ "name": name }))
        .await?;

    assert_eq!(
        resource_tag_edge(&author, &tag.file_uri).await?,
        None,
        "the removed tag must leave the graph"
    );
    assert_feed_matches_graph(&author, start, &tag.file_uri).await?;
    assert!(
        authored_since(&author, start)
            .await?
            .iter()
            .any(|line| line.op == ResourceChangeOp::Del && line.object_path == tag.v0_path),
        "the removal retracts the pubky.app path"
    );
    let recorded = recorded_moves(&author).await?;
    assert!(recorded.is_empty(), "{recorded:?}");
    let v0_uri = format!("pubky://{author}{}", tag.v0_path);
    assert_eq!(
        ResourceChange::aliased_delete(&author, &v0_uri).await?,
        None
    );
    test.del(&key, &tag.v1_storage).await?;
    test.del(&key, &profile).await?;
    test.cleanup_file(&key, &tag.file_path).await?;
    Ok(())
}

/// The user deletes the moved own-file tag at its `pubky.app` address.
#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_delete_of_a_moved_own_file_tag_before_the_flip_retry() -> Result<()>
{
    own_file_tag_removed_before_the_flip_retry(
        "Own file delete before flip retry",
        async |test, key, author, tag| {
            test.del(key, &tag.v0_path.parse()?).await?;
            retrieve_and_handle_event_line(
                &format!("DEL pubky://{author}{}", tag.v0_path),
                test.event_processor_runner.event_handler.clone(),
            )
            .await?;
            Ok(())
        },
    )
    .await
}

/// A moderator takes the moved own-file tag down at its `pubky.app` address.
#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_takedown_of_a_moved_own_file_tag_before_the_flip_retry() -> Result<()>
{
    use super::moderated::{create_moderator, moderate_tag};

    own_file_tag_removed_before_the_flip_retry(
        "Own file takedown before flip retry",
        async |test, _, author, tag| {
            let moderator = create_moderator(test).await?;
            moderate_tag(
                test,
                &moderator,
                &format!("pubky://{author}{}", tag.v0_path),
            )
            .await
        },
    )
    .await
}

/// A takedown of the migrator's `social/v1` copy of an own-file tag is keyed by the copy's id,
/// the id the flip gives the edge. The flip's tags step holds that lock too while it moves the
/// edge: here the test holds it as the takedown does, the flip waits, and the takedown runs
/// after the move. The feed then holds what the graph holds.
#[tokio_shared_rt::test(shared)]
async fn test_resource_tag_flip_waits_for_the_lock_of_the_new_id() -> Result<()> {
    use futures::FutureExt;
    use nexus_watcher::events::handlers::tag;
    use nexus_watcher::events::inputs::ResourceKey;

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Flip new id lock", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let own = put_own_file_tag(&mut test, &key, &author).await?;
    let v1_uri = format!("pubky://{author}{}", own.v1_storage);

    let guard = tag::lock_tag(&author, &own.v1_id).await;
    let mut flip = flip_tags(&author, 2)?;
    // Bounds the wait only: a flip that does not take the lock finishes well within it
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(2), &mut flip)
            .await
            .is_err(),
        "the flip must wait while the new id's lock is held"
    );
    let takedown_key = ResourceKey {
        owner_id: crate::event_processor::posts::utils::pubky_id(&author)?,
        resource_id: own.v1_id.clone(),
    };
    let mut takedown = async move { tag::del_at(takedown_key, &v1_uri).await }.boxed_local();
    queue_tag_handlers(&[TagStep::Del], std::slice::from_mut(&mut takedown)).await;
    drop(guard);
    let (flipped, taken_down) = futures::join!(flip, takedown);
    flipped?;
    taken_down?;

    assert_eq!(resource_tag_edge(&author, &own.file_uri).await?, None);
    assert_feed_matches_graph(&author, start, &own.file_uri).await?;
    let recorded = recorded_moves(&author).await?;
    assert!(recorded.is_empty(), "{recorded:?}");
    test.del(&key, &own.v1_storage).await?;
    test.cleanup_file(&key, &own.file_path).await?;
    test.cleanup_user(&key).await?;
    Ok(())
}

/// A DEL queued for a tag file's lock ahead of the flip, after the flip's first read listed the
/// file, runs first: the flip reads its page again under the lock, finds the edge gone, and
/// moves nothing.
#[tokio_shared_rt::test(shared)]
async fn test_resource_tag_flip_rereads_a_page_a_delete_changed_before_the_grant() -> Result<()> {
    use nexus_watcher::events::handlers::tag;

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Flip delete before grant", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let target = format!("https://example.com/flip-delete-before-grant/{author}");
    run_tag_steps(&author, &target, &[TagStep::Put]).await?;
    let id = PubkyAppTag::new(target.clone(), "steps".into()).create_id();

    let guard = tag::lock_tag(&author, &id).await;
    let mut delete = [tag_handler(&author, &target, TagStep::Del)?];
    queue_tag_handlers(&[TagStep::Del], &mut delete).await;
    let [delete] = delete;
    let mut flip = flip_tags(&author, 2)?;
    // Bounds the wait only: the flip reads its page, then waits for the lock
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(2), &mut flip)
            .await
            .is_err(),
        "the flip must wait while the tag's lock is held"
    );
    drop(guard);
    let (deleted, flipped) = futures::join!(delete, flip);
    deleted?;
    flipped?;

    assert_eq!(resource_tag_edge(&author, &target).await?, None);
    assert_feed_matches_graph(&author, start, &target).await?;
    let recorded = recorded_moves(&author).await?;
    assert!(recorded.is_empty(), "{recorded:?}");
    test.cleanup_user(&key).await?;
    Ok(())
}

/// An edge created after the flip's first read and before its grant is in the page the flip
/// reads again under the lock, but not under the locks it took: the flip takes them again with
/// that file's, so it waits while that file's lock is held, and then moves both edges.
#[tokio_shared_rt::test(shared)]
async fn test_resource_tag_flip_relocks_for_an_edge_created_before_the_grant() -> Result<()> {
    use nexus_watcher::events::handlers::tag;

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Flip edge before grant", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let first = format!("https://example.com/flip-edge-before-grant/first/{author}");
    run_tag_steps(&author, &first, &[TagStep::Put]).await?;
    let id = PubkyAppTag::new(first.clone(), "steps".into()).create_id();
    let stripe = tag::tag_lock_stripe(&author, &id);
    // A second file whose lock is not the one the test holds
    let second = (0..)
        .map(|n| format!("https://example.com/flip-edge-before-grant/{n}/{author}"))
        .find(|target| {
            let id = PubkyAppTag::new(target.clone(), "steps".into()).create_id();
            tag::tag_lock_stripe(&author, &id) != stripe
        })
        .unwrap();

    let guard = tag::lock_tag(&author, &id).await;
    let mut flip = flip_tags(&author, 2)?;
    // Bounds the wait only: the flip reads its page, then waits for the lock
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(2), &mut flip)
            .await
            .is_err(),
        "the flip must wait while the tag's lock is held"
    );
    run_tag_steps(&author, &second, &[TagStep::Put]).await?;
    let second_id = PubkyAppTag::new(second.clone(), "steps".into()).create_id();
    let second_guard = tag::lock_tag(&author, &second_id).await;
    drop(guard);
    // Bounds the wait only: a flip that moved the second edge without its lock finishes well
    // within it
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(2), &mut flip)
            .await
            .is_err(),
        "the flip must wait for the lock of the edge created before its grant"
    );
    drop(second_guard);
    flip.await?;

    for target in [&first, &second] {
        assert!(
            resource_tag_edge(&author, target)
                .await?
                .is_some_and(|uri| uri.contains("/pub/social/v1/tags/")),
            "{target} must have moved"
        );
        assert_feed_matches_graph(&author, start, target).await?;
    }
    let recorded = recorded_moves(&author).await?;
    assert!(recorded.is_empty(), "{recorded:?}");
    test.cleanup_user(&key).await?;
    Ok(())
}

/// The DEL of a moved own-file tag at its `pubky.app` address, as the handler runs it up to a
/// publish that fails: its journal under the address on the edge, named by the `pubky.app`
/// address, then the graph delete.
async fn fail_to_publish_a_delete_of_a_moved_own_file_tag(
    author: &str,
    tag: &OwnFileTag,
) -> Result<()> {
    use nexus_common::db::{fetch_row_from_graph, queries};

    let v0_uri = format!("pubky://{author}{}", tag.v0_path);
    let (normalized, _) = normalize_uri(&tag.file_uri).unwrap();
    let del = |path: String| {
        ResourceChange::tag(
            ResourceChangeOp::Del,
            resource_id(&normalized),
            normalized.to_string(),
            author.to_string(),
            "steps".into(),
            &format!("pubky://{author}{path}"),
            Utc::now().timestamp_millis(),
        )
    };
    // The del of the address the edge moved from, while its move is recorded
    let moved = match recorded_moves(author).await?.is_empty() {
        true => None,
        false => Some(del(tag.v0_path.clone())?),
    };
    del(tag.v1_storage.to_string())?
        .prepare_aliased_delete(&v0_uri, moved.as_ref())
        .await?;
    let v0_id = tag.v0_path.rsplit('/').next().unwrap();
    fetch_row_from_graph(queries::del::delete_tag(author, v0_id)).await?;
    Ok(())
}

/// [`fail_to_publish_a_delete_of_a_moved_own_file_tag`], then its retry, which finds no edge.
async fn delete_a_moved_own_file_tag_failing_to_publish(
    test: &mut WatcherTest,
    key: &Keypair,
    author: &str,
    tag: &OwnFileTag,
) -> Result<()> {
    fail_to_publish_a_delete_of_a_moved_own_file_tag(author, tag).await?;
    // Its retry
    test.del(key, &tag.v0_path.parse()?).await?;
    retrieve_and_handle_event_line(
        &format!("DEL pubky://{author}{}", tag.v0_path),
        test.event_processor_runner.event_handler.clone(),
    )
    .await?;
    Ok(())
}

/// The user deletes the moved own-file tag at its `pubky.app` address after its move was
/// published, the DEL fails after its graph delete, and the user tags the file again at that
/// address before the DEL's retry. The retry finds and deletes the new edge, and finishes the
/// journal its first attempt left under the moved address: the consumer holds that address.
#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_delete_retry_on_a_tag_put_again_finishes_its_first_journal(
) -> Result<()> {
    own_file_tag_removed_after_a_failed_flip(
        "Own file delete retry after a put again",
        true,
        async |test, key, author, tag| {
            fail_to_publish_a_delete_of_a_moved_own_file_tag(author, tag).await?;
            let again = PubkyAppTag::new(tag.file_uri.clone(), "steps".into());
            test.put(key, &tag.v0_path.parse()?, &again).await?;
            assert_eq!(
                resource_tag_edge(author, &tag.file_uri).await?,
                Some(format!("pubky://{author}{}", tag.v0_path))
            );
            // The DEL's retry
            retrieve_and_handle_event_line(
                &format!("DEL pubky://{author}{}", tag.v0_path),
                test.event_processor_runner.event_handler.clone(),
            )
            .await?;
            let lines = authored_since(author, 0).await?;
            assert!(
                live_references(&lines).is_empty(),
                "the retry must retract both addresses: {lines:?}"
            );
            assert_eq!(
                ResourceChange::aliased_delete(author, &format!("pubky://{author}{}", tag.v0_path))
                    .await?,
                None
            );
            test.del(key, &tag.v0_path.parse()?).await?;
            Ok(())
        },
    )
    .await
}

/// The user deletes the moved own-file tag at its `pubky.app` address before the failed flip
/// published its move, and the DEL fails after its graph delete, before it publishes. Its retry
/// finds no edge, and publishes the del of the moved address through the alias the DEL left,
/// with the del of the `pubky.app` address pending with it.
#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_delete_of_a_moved_own_file_tag_that_fails_to_publish() -> Result<()>
{
    own_file_tag_removed_after_a_failed_flip(
        "Own file delete fails to publish",
        false,
        delete_a_moved_own_file_tag_failing_to_publish,
    )
    .await
}

/// [`test_resource_changes_delete_of_a_moved_own_file_tag_that_fails_to_publish`] after the
/// failed flip published its move: the move is no longer recorded, and the retry publishes the
/// del of the moved address through the alias alone.
#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_delete_of_a_published_own_file_tag_move_that_fails_to_publish(
) -> Result<()> {
    own_file_tag_removed_after_a_failed_flip(
        "Own file delete after publish fails to publish",
        true,
        delete_a_moved_own_file_tag_failing_to_publish,
    )
    .await
}

/// A DEL of a moved own-file tag deleted the edge but failed to publish, leaving its del
/// pending under the moved address, with the del of the `pubky.app` address. The retried
/// flip's tags step then finds no edge to move, and before it publishes, the user, still `v0`,
/// tags the file again at the `pubky.app` address: the new edge is live, so the flip's publish
/// must not retract that address.
#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_flip_publish_keeps_a_tag_put_again_after_a_failed_delete(
) -> Result<()> {
    use crate::event_processor::posts::utils::pubky_id;
    use nexus_common::db::{fetch_row_from_graph, queries};
    use nexus_watcher::events::v0_to_v1;

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Own file put after failed delete", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let tag = put_own_file_tag(&mut test, &key, &author).await?;
    let author_id = pubky_id(&author)?;
    let file_map = v0_to_v1::files::FileMap::load(&author_id).await?;
    v0_to_v1::tags::migrate(&author_id, &file_map, 2).await?;

    // The DEL's journal under the moved address, with the del of the address the edge moved
    // from, and its graph delete; its publish fails
    let v0_uri = format!("pubky://{author}{}", tag.v0_path);
    let (normalized, _) = normalize_uri(&tag.file_uri).unwrap();
    let del = |path: String| {
        ResourceChange::tag(
            ResourceChangeOp::Del,
            resource_id(&normalized),
            normalized.to_string(),
            author.clone(),
            "steps".into(),
            &format!("pubky://{author}{path}"),
            Utc::now().timestamp_millis(),
        )
    };
    del(tag.v1_storage.to_string())?
        .prepare_aliased_delete(&v0_uri, Some(&del(tag.v0_path.clone())?))
        .await?;
    let v0_id = tag.v0_path.rsplit('/').next().unwrap();
    fetch_row_from_graph(queries::del::delete_tag(&author, v0_id)).await?;

    // The retried flip's tags step, then a new PUT of the `pubky.app` file, then its publish
    v0_to_v1::tags::migrate(&author_id, &file_map, 2).await?;
    let again = PubkyAppTag::new(tag.file_uri.clone(), "steps".into());
    test.put(&key, &tag.v0_path.parse()?, &again).await?;
    v0_to_v1::resource_changes::publish_moves(&author_id).await?;

    assert_eq!(
        resource_tag_edge(&author, &tag.file_uri).await?,
        Some(format!("pubky://{author}{}", tag.v0_path))
    );
    assert_feed_matches_graph(&author, start, &tag.file_uri).await?;
    test.cleanup_file(&key, &tag.file_path).await?;
    test.cleanup_user(&key).await?;
    Ok(())
}

/// A takedown of a moved own-file tag at its `social/v1` address, while the flip that moved it
/// waits for its retry, journals the del of the `pubky.app` address beside its own and fails
/// to publish after its graph delete. The user, still `v0`, then tags the file again at the
/// `pubky.app` address. That put makes the address live again, so the takedown's retry, which
/// finds no edge, must publish only the del of the `social/v1` address.
#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_put_at_the_old_address_drops_a_pending_moved_del() -> Result<()> {
    use crate::event_processor::posts::utils::pubky_id;
    use deadpool_redis::redis::AsyncCommands;
    use nexus_common::db::get_redis_conn;
    use nexus_watcher::events::handlers::tag;
    use nexus_watcher::events::inputs::ResourceKey;
    use nexus_watcher::events::v0_to_v1;

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Put at the old address", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let tag = put_own_file_tag(&mut test, &key, &author).await?;
    let author_id = pubky_id(&author)?;
    let file_map = v0_to_v1::files::FileMap::load(&author_id).await?;
    v0_to_v1::tags::migrate(&author_id, &file_map, 2).await?;
    let v1_uri = format!("pubky://{author}{}", tag.v1_storage);
    let takedown_key = || ResourceKey {
        owner_id: author_id.clone(),
        resource_id: tag.v1_id.clone(),
    };

    // The takedown fails after its graph delete: its path's journal key holds another type
    let journal = format!(
        "Event:ResourceChanges:PendingDelete:{author}:{}",
        tag.v1_storage
    );
    let mut conn = get_redis_conn().await?;
    let _: () = conn.hset(&journal, "of", "another type").await?;
    assert!(
        tag::del_at(takedown_key(), &v1_uri).await.is_err(),
        "the takedown must fail to publish"
    );
    assert_eq!(resource_tag_edge(&author, &tag.file_uri).await?, None);
    // The journal as the takedown wrote it, the del of its address
    let _: () = conn.del(&journal).await?;
    let (normalized, _) = normalize_uri(&tag.file_uri).unwrap();
    ResourceChange::tag(
        ResourceChangeOp::Del,
        resource_id(&normalized),
        normalized.to_string(),
        author.clone(),
        "steps".into(),
        &v1_uri,
        Utc::now().timestamp_millis(),
    )?
    .prepare_delete(None)
    .await?;

    // The same tag again, at the `pubky.app` address, then the takedown's retry
    let again = PubkyAppTag::new(tag.file_uri.clone(), "steps".into());
    test.put(&key, &tag.v0_path.parse()?, &again).await?;
    assert_eq!(
        ResourceChange::pending_moved_from(&author, &v1_uri).await?,
        Some(format!("pubky://{author}{}", tag.v0_path)),
        "the takedown journals the del of the pubky.app address"
    );
    let (_, _, before_retry) = ResourceChange::get_from_index(0, 0).await?;
    tag::del_at(takedown_key(), &v1_uri).await?;
    assert_eq!(
        authored_since(&author, before_retry)
            .await?
            .iter()
            .map(|line| (line.op, line.object_path.clone()))
            .collect::<Vec<_>>(),
        [(ResourceChangeOp::Del, tag.v1_storage.to_string())],
        "the retry publishes only the del of the social/v1 address"
    );

    assert_eq!(
        resource_tag_edge(&author, &tag.file_uri).await?,
        Some(format!("pubky://{author}{}", tag.v0_path))
    );
    assert_feed_matches_graph(&author, start, &tag.file_uri).await?;
    test.cleanup_file(&key, &tag.file_path).await?;
    test.cleanup_user(&key).await?;
    Ok(())
}

/// The DEL of a moved own-file tag at its `pubky.app` address names its journal from that
/// address before the graph delete. The test makes the DEL fail after its graph delete: it
/// seeds the moved address's journal key with a value of another type, which the journal step
/// leaves in place and the publish step cannot read. The alias the journal step wrote stays,
/// and the DEL's retry, once the key is cleared, removes it.
#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_delete_of_a_moved_own_file_tag_names_its_journal() -> Result<()> {
    use crate::event_processor::posts::utils::pubky_id;
    use deadpool_redis::redis::AsyncCommands;
    use nexus_common::db::get_redis_conn;
    use nexus_watcher::events::handlers::tag;
    use nexus_watcher::events::inputs::ResourceKey;
    use nexus_watcher::events::v0_to_v1;

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Own file delete alias", "feed"))
        .await?;
    let tag = put_own_file_tag(&mut test, &key, &author).await?;
    let author_id = pubky_id(&author)?;
    let file_map = v0_to_v1::files::FileMap::load(&author_id).await?;
    v0_to_v1::tags::migrate(&author_id, &file_map, 2).await?;
    let v0_uri = format!("pubky://{author}{}", tag.v0_path);
    let v1_uri = format!("pubky://{author}{}", tag.v1_storage);
    let v0_key = || -> Result<ResourceKey> {
        Ok(ResourceKey {
            owner_id: author_id.clone(),
            resource_id: tag.v0_path.rsplit('/').next().unwrap().to_string(),
        })
    };

    let journal = format!(
        "Event:ResourceChanges:PendingDelete:{author}:{}",
        tag.v1_storage
    );
    let mut conn = get_redis_conn().await?;
    let _: () = conn.hset(&journal, "of", "another type").await?;
    assert!(
        tag::del_at(v0_key()?, &v0_uri).await.is_err(),
        "the DEL must fail to publish"
    );
    assert_eq!(resource_tag_edge(&author, &tag.file_uri).await?, None);
    assert_eq!(
        ResourceChange::aliased_delete(&author, &v0_uri).await?,
        Some(v1_uri),
        "the DEL must name its journal from the deleted file's address"
    );

    let _: () = conn.del(&journal).await?;
    tag::del_at(v0_key()?, &v0_uri).await?;
    assert_eq!(
        ResourceChange::aliased_delete(&author, &v0_uri).await?,
        None
    );
    test.cleanup_file(&key, &tag.file_path).await?;
    test.cleanup_user(&key).await?;
    Ok(())
}
