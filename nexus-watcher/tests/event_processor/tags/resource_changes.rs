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

/// The live tag references a consumer applying `lines` in order holds, by the feed's key for
/// tags, `(author, uri, label)`.
pub(super) fn live_references(
    lines: &[ResourceChange],
) -> std::collections::BTreeSet<(String, String, String)> {
    let mut live = std::collections::BTreeSet::new();
    for line in lines {
        let key = (
            line.author.clone(),
            line.uri.clone(),
            line.label.clone().unwrap_or_default(),
        );
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

    // A further retry is a duplicate put, harmless to a consumer keyed by (author, uri, label).
    retrieve_and_handle_event_line(&event, handler.clone()).await?;
    let lines = authored_since(&author, start).await?;
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], lines[1]);
    assert_eq!(lines[0], recovered[0]);
    assert!(lines.iter().all(|line| line.op == ResourceChangeOp::Put));
    assert_eq!(
        live_references(&lines),
        [(author.clone(), uri.clone(), tag.label.clone())].into()
    );

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
    // Both files publish the one reference; the delete of the file whose id the edge carries
    // retracts it, and the other file's delete finds no edge
    assert_eq!(
        lines
            .iter()
            .map(|line| (line.op, line.object_path.as_str()))
            .collect::<Vec<_>>(),
        [
            (ResourceChangeOp::Put, files[0].0.as_str()),
            (ResourceChangeOp::Put, files[1].0.as_str()),
            (ResourceChangeOp::Del, files[0].0.as_str()),
        ],
        "{lines:?}"
    );
    assert!(lines.windows(2).all(|pair| pair[0].uri == pair[1].uri));
    assert!(live_references(&lines).is_empty(), "{lines:?}");
    test.cleanup_user(&key).await?;
    Ok(())
}

/// A tag's `pubky.app` file and its `social/v1` copy share one edge (one tag id). The copy is
/// gated until the profile flip, which moves the edge to the copy's address and appends
/// nothing: the reference is the same `(author, uri, label)`. Deleting the v1 file retracts
/// it, and a consumer ends with nothing live.
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
    let before_flip = authored_since(&author, start).await?;
    assert!(before_flip.iter().all(|line| line.object_path == v0_path));
    let profile: ResourcePath = "/pub/social/v1/profile.json".parse()?;
    test.put(&key, &profile, &serde_json::json!({"name":"Tag copies"}))
        .await?;
    assert_eq!(
        authored_since(&author, start).await?,
        before_flip,
        "the flip appends nothing"
    );
    // Handled at v1 now: an upsert of the same reference, published under its v1 path
    retrieve_and_handle_event_line(&format!("PUT {v1_uri}"), handler.clone()).await?;
    test.del(&key, &v1_storage).await?;
    retrieve_and_handle_event_line(&format!("DEL {v1_uri}"), handler.clone()).await?;
    // Gated at v1: publishes nothing
    test.del(&key, &v0_storage).await?;
    retrieve_and_handle_event_line(&format!("DEL {v0_uri}"), handler.clone()).await?;

    let lines = authored_since(&author, start).await?;
    assert!(
        lines[before_flip.len()..]
            .iter()
            .all(|line| line.object_path == v1_path),
        "after the flip only the v1 file is published: {lines:?}"
    );
    assert!(lines.iter().all(|line| line.uri == before_flip[0].uri));
    assert_eq!(
        lines.last().map(|line| line.op),
        Some(ResourceChangeOp::Del)
    );
    assert!(live_references(&lines).is_empty(), "{lines:?}");
    test.del(&key, &profile).await?;
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

/// Polls each of `handlers` once, asserting it is pending: each queues for its tagger's lock in
/// that order, while the caller holds it. (Pending alone does not prove the wait: the lock tests
/// below guard that the handlers take the lock.)
async fn queue_tag_handlers(
    steps: &[TagStep],
    handlers: &mut [LocalBoxFuture<'static, Result<(), EventProcessorError>>],
) {
    for (step, handler) in steps.iter().zip(handlers.iter_mut()) {
        assert!(
            futures::poll!(handler.as_mut()).is_pending(),
            "{step:?} must be pending while the tagger's lock is held"
        );
    }
}

/// Queues `steps` for one tag file behind its tagger's lock, in that order, then lets them run: the
/// lock grants them one at a time, first come first served. Each step is polled until it waits
/// for the lock, so the order is fixed before any of them runs. Returns the file's path.
async fn run_tag_steps(author: &str, target: &str, steps: &[TagStep]) -> Result<String> {
    use futures::future::join_all;
    use nexus_watcher::events::handlers::tag;

    let id = PubkyAppTag::new(target.to_string(), "steps".into()).create_id();
    let path = format!("/pub/pubky.app/tags/{id}");
    let guard = tag::lock_tagger(author).await;
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

/// The address stored on `author`'s `steps` edge to `target`, if the edge exists.
async fn resource_tag_edge(author: &str, target: &str) -> Result<Option<String>> {
    use nexus_common::db::{fetch_row_from_graph, graph::Query};

    let (normalized, _) = normalize_uri(target).unwrap();
    let query = Query::new(
        "test_resource_tag_edge",
        "MATCH (:User {id: $author})-[t:TAGGED {label: 'steps'}]->(:Resource {id: $resource_id})
         RETURN t.uri AS tag_uri",
    )
    .param("author", author)
    .param("resource_id", resource_id(&normalized));
    let edge = fetch_row_from_graph(query).await?;
    Ok(edge.map(|row| row.get("tag_uri")).transpose()?)
}

/// The feed's live references of `author` to `target` since `start` are exactly the graph's:
/// the edge's reference when it exists, none otherwise.
async fn assert_feed_matches_graph(author: &str, start: u64, target: &str) -> Result<()> {
    let (normalized, _) = normalize_uri(target).unwrap();
    let rid = resource_id(&normalized);
    let edge = resource_tag_edge(author, target).await?;
    let lines: Vec<_> = authored_since(author, start)
        .await?
        .into_iter()
        .filter(|line| line.resource_id == rid)
        .collect();
    let expected: std::collections::BTreeSet<_> = match edge {
        Some(_) => [(author.to_string(), normalized, "steps".to_string())].into(),
        None => Default::default(),
    };
    assert_eq!(live_references(&lines), expected, "feed {lines:?}");
    Ok(())
}

/// A DEL of a resource tag waits while another handler of the same tagger holds the tagger's lock,
/// and runs once it is released.
#[tokio_shared_rt::test(shared)]
async fn test_resource_tag_delete_waits_for_the_tagger_lock() -> Result<()> {
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

    let guard = tag::lock_tagger(&author).await;
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
        "a DEL must wait while the tagger's lock is held"
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

/// A PUT of a resource tag takes the tagger's lock before its graph write: while another
/// handler of the tagger holds the lock, the PUT writes and appends nothing, and it runs once
/// the lock is released. (The PUT and DEL ordering tests cover what it does while holding it.)
#[tokio_shared_rt::test(shared)]
async fn test_resource_tag_put_waits_for_the_tagger_lock() -> Result<()> {
    use nexus_watcher::events::handlers::tag;
    use nexus_watcher::events::inputs::{Reference, ReferenceTarget, ResourceKey, TagInput};

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Tag put lock", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let target = format!("https://example.com/tag-put-lock/{author}");
    let id = PubkyAppTag::new(target.clone(), "steps".into()).create_id();
    let input = TagInput {
        uri: format!("pubky://{author}/pub/pubky.app/tags/{id}"),
        tagged: Reference {
            uri: target.clone(),
            target: ReferenceTarget::Foreign,
        },
        label: "steps".into(),
    };
    let tag_key = ResourceKey {
        owner_id: crate::event_processor::posts::utils::pubky_id(&author)?,
        resource_id: id.clone(),
    };

    let guard = tag::lock_tagger(&author).await;
    let mut put = tokio::spawn(tag::sync_put_resource(input, tag_key));
    // Bounds the wait only: a PUT that does not take the lock finishes well within it
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(2), &mut put)
            .await
            .is_err(),
        "a PUT must wait while the tagger's lock is held"
    );
    assert_eq!(resource_tag_edge(&author, &target).await?, None);
    assert!(authored_since(&author, start).await?.is_empty());
    drop(guard);
    put.await??;
    assert_feed_matches_graph(&author, start, &target).await?;
    let lines = authored_since(&author, start).await?;
    assert_eq!(
        lines.iter().map(|line| line.op).collect::<Vec<_>>(),
        [ResourceChangeOp::Put]
    );
    run_tag_steps(&author, &target, &[TagStep::Del]).await?;
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

/// A resource tag edge whose stored address is outside its author's folder (no writer stores
/// one; a repaired or imported edge could hold it) is still deleted by the DEL of its file: the
/// del line takes its `object_path` from the event's address, never from the edge.
#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_delete_ignores_a_bad_stored_address() -> Result<()> {
    use nexus_common::db::{exec_single_row, graph::Query};

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Bad stored address", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let target = format!("https://example.com/bad-stored-address/{author}");
    let tag = PubkyAppTag::new(target.clone(), "steps".into());
    let path = format!("/pub/pubky.app/tags/{}", tag.create_id());
    let storage: ResourcePath = path.parse()?;
    let tag_uri = format!("pubky://{author}{path}");
    test.put(&key, &storage, &tag).await?;
    let other = Keypair::random().public_key().to_z32();
    exec_single_row(
        Query::new(
            "test_bad_stored_address",
            "MATCH (:User {id: $author})-[t:TAGGED {label: 'steps'}]->(:Resource)
             SET t.uri = $bad",
        )
        .param("author", author.as_str())
        .param(
            "bad",
            format!("pubky://{other}/pub/pubky.app/tags/elsewhere"),
        ),
    )
    .await?;

    retrieve_and_handle_event_line(
        &format!("DEL {tag_uri}"),
        test.event_processor_runner.event_handler.clone(),
    )
    .await?;
    assert_eq!(resource_tag_edge(&author, &target).await?, None);
    let lines = authored_since(&author, start).await?;
    let del = lines.last().expect("the delete publishes its del");
    assert_eq!(
        (del.op, del.object_path.as_str()),
        (ResourceChangeOp::Del, path.as_str())
    );
    assert!(live_references(&lines).is_empty(), "{lines:?}");
    test.cleanup_user(&key).await?;
    Ok(())
}

/// The flip's tags step holds the tagger's lock for each batch, from its read to its write: while
/// another handler of the tagger holds it, the step moves nothing, and it runs once the lock is
/// released. A DEL therefore never reads an edge, appends its del and then misses the edge the
/// step gave a new id.
#[tokio_shared_rt::test(shared)]
async fn test_resource_tag_flip_waits_for_the_tagger_lock() -> Result<()> {
    use nexus_watcher::events::{handlers::tag, v0_to_v1};

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Flip lock", "feed"))
        .await?;
    let target = format!("https://example.com/flip-lock/{author}");
    let path = run_tag_steps(&author, &target, &[TagStep::Put]).await?;
    assert_eq!(
        resource_tag_edge(&author, &target).await?,
        Some(format!("pubky://{author}{path}"))
    );

    let user = crate::event_processor::posts::utils::pubky_id(&author)?;
    let files = v0_to_v1::files::FileMap::load(&user).await?;
    let guard = tag::lock_tagger(&author).await;
    let flip = v0_to_v1::tags::migrate(&user, &files, v0_to_v1::tags::BATCH_SIZE);
    tokio::pin!(flip);
    // Bounds the wait only: a step that does not take the lock finishes well within it
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(2), &mut flip)
            .await
            .is_err(),
        "the tags step must wait while the tagger's lock is held"
    );
    assert_eq!(
        resource_tag_edge(&author, &target).await?,
        Some(format!("pubky://{author}{path}")),
        "nothing moves while the lock is held"
    );
    drop(guard);
    flip.await?;
    let moved = resource_tag_edge(&author, &target)
        .await?
        .expect("the edge");
    assert!(moved.contains("/pub/social/v1/tags/"), "{moved}");
    test.cleanup_user(&key).await?;
    Ok(())
}
