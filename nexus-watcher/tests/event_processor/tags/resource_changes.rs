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
    let (outcome, _) = TagResource::put_to_graph_resource(
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

/// Two equivalent tag files of one tagger (two spellings of one URL, so two file ids) land on
/// one edge, which carries the first file's id. Both puts publish the one `(author, uri,
/// label)` reference; deleting the file whose id the edge carries removes the edge and
/// publishes one del, and the other file's delete finds no edge and publishes nothing.
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
    let mut lines = authored_since(&author, start).await?;
    // Each PUT is handled twice (by the test's write and by the replay): duplicates
    lines.dedup();
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
    assert!(lines.windows(2).all(|pair| pair[0].uri == pair[1].uri
        && pair[0].label == pair[1].label
        && pair[0].resource_id == pair[1].resource_id));
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
    // The v1 DEL retracts the reference the pubky.app put published
    let (put, del) = (&before_flip[0], lines.last().unwrap());
    assert_eq!(del.op, ResourceChangeOp::Del);
    assert_eq!(
        (&del.author, &del.uri, &del.label, &del.resource_id),
        (&put.author, &put.uri, &put.label, &put.resource_id)
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

/// Gives `author`'s `steps` edge to `target` the id `id` and the `social/v1` address built from it,
/// with the flip's own write (`set_tag_addresses`, which the tags step runs under the tagger's
/// lock): the same tagger, target and label, at a new address.
async fn move_resource_tag_edge(author: &str, target: &str, id: &str) -> Result<String> {
    use nexus_common::db::{fetch_key_from_graph, fetch_row_from_graph, graph::Query, queries};

    let (normalized, _) = normalize_uri(target).unwrap();
    let edge: String = fetch_row_from_graph(
        Query::new(
            "test_resource_tag_edge_element",
            "MATCH (:User {id: $author})-[t:TAGGED {label: 'steps'}]->(:Resource {id: $resource_id})
             RETURN elementId(t) AS edge",
        )
        .param("author", author)
        .param("resource_id", resource_id(&normalized)),
    )
    .await?
    .expect("the edge")
    .get("edge")?;
    let uri = pubky_social_specs::tag_uri_builder(author.to_string(), id.to_string());
    let changed = fetch_key_from_graph::<i64>(
        queries::migration::set_tag_addresses(author, &[(edge, id.to_string(), uri.clone())]),
        "changed",
    )
    .await?;
    assert_eq!(changed, Some(1));
    Ok(uri)
}

/// A DEL of a resource tag reads the edge only once it holds the tagger's lock. While another
/// handler of the tagger holds the lock, the DEL waits; meanwhile the edge moves to the address
/// the DEL names, as the flip's tags step moves it (new id and `social/v1` address, same tagger,
/// target and label). Once released, the DEL finds and deletes the edge, and its del follows
/// the graph. A DEL that read the edge before taking the lock would have found none at that id
/// and left the moved edge in the graph.
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
    assert_eq!(
        resource_tag_edge(&author, &target).await?,
        Some(tag_uri.clone())
    );
    // Another id of the tagger's, standing in for the one the flip gives the edge
    let moved_id = PubkyAppTag::new(format!("{target}#moved"), "steps".into()).create_id();
    assert_ne!(moved_id, id);
    let moved_uri = pubky_social_specs::tag_uri_builder(author.clone(), moved_id.clone());

    let guard = tag::lock_tagger(&author).await;
    let owner_id = crate::event_processor::posts::utils::pubky_id(&author)?;
    let (del_id, del_uri) = (moved_id.clone(), moved_uri.clone());
    let mut delete = tokio::spawn(async move {
        tag::del_at(
            ResourceKey {
                owner_id,
                resource_id: del_id,
            },
            &del_uri,
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
    assert_eq!(
        move_resource_tag_edge(&author, &target, &moved_id).await?,
        moved_uri
    );
    drop(guard);
    delete.await??;
    assert_eq!(
        resource_tag_edge(&author, &target).await?,
        None,
        "the DEL deletes the edge it finds under the lock"
    );
    assert_feed_matches_graph(&author, start, &target).await?;
    let lines = authored_since(&author, start).await?;
    let del = lines.last().expect("the DEL appends its del");
    assert_eq!(del.op, ResourceChangeOp::Del);
    assert!(
        moved_uri.ends_with(&del.object_path),
        "{} is the DEL's file",
        del.object_path
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
/// step gave a new id. While the step waits, the lock's holder deletes the edge (the graph
/// write of a DEL): the step then reads its page under the lock, finds nothing left to move
/// and succeeds. A step that read its page before taking the lock would write a page of edges
/// that are gone, and fail.
#[tokio_shared_rt::test(shared)]
async fn test_resource_tag_flip_waits_for_the_tagger_lock() -> Result<()> {
    use nexus_common::db::{fetch_row_from_graph, queries};
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
    let id = PubkyAppTag::new(target.clone(), "steps".into()).create_id();
    assert!(path.ends_with(&id), "{path}");
    fetch_row_from_graph(queries::del::delete_tag(&author, &id)).await?;
    assert_eq!(resource_tag_edge(&author, &target).await?, None);
    drop(guard);
    flip.await
        .map_err(|e| anyhow::anyhow!("the step must read its page under the lock: {e}"))?;
    assert_eq!(
        resource_tag_edge(&author, &target).await?,
        None,
        "the step moves no deleted edge"
    );

    // With the edge in place, the released step moves it.
    let path = run_tag_steps(&author, &target, &[TagStep::Put]).await?;
    assert!(path.contains("/pub/pubky.app/tags/"), "{path}");
    v0_to_v1::tags::migrate(&user, &files, v0_to_v1::tags::BATCH_SIZE).await?;
    let moved = resource_tag_edge(&author, &target)
        .await?
        .expect("the edge");
    assert!(moved.contains("/pub/social/v1/tags/"), "{moved}");
    test.cleanup_user(&key).await?;
    Ok(())
}

/// A `pubky.app` resource tag whose target is longer than the spec's reference cap is rejected
/// with a spec validation error, which the watcher does not retry: no edge and no feed line. A
/// target at the cap is indexed and published. (A `social/v1` tag over the cap fails the spec's
/// own validation before it reaches the handler.)
#[tokio_shared_rt::test(shared)]
async fn test_resource_tag_target_over_the_cap_is_rejected() -> Result<()> {
    use nexus_common::models::resource::tag::RESOURCE_TAG_TARGET_MAX_LENGTH;

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Long target", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let handler = test.event_processor_runner.event_handler.clone();
    let prefix = format!("https://example.com/long-target/{author}/");
    let target = |length: usize| format!("{prefix}{}", "a".repeat(length - prefix.len()));
    for (length, accepted) in [
        (RESOURCE_TAG_TARGET_MAX_LENGTH, true),
        (RESOURCE_TAG_TARGET_MAX_LENGTH + 1, false),
    ] {
        let target = target(length);
        let tag = PubkyAppTag::new(target.clone(), "steps".into());
        let path = format!("/pub/pubky.app/tags/{}", tag.create_id());
        test.put(&key, &path.parse::<ResourcePath>()?, &tag).await?;
        let handled =
            retrieve_and_handle_event_line(&format!("PUT pubky://{author}{path}"), handler.clone())
                .await;
        let edge = resource_tag_edge(&author, &target).await?;
        if accepted {
            handled?;
            assert!(edge.is_some(), "a target at the cap is indexed");
        } else {
            assert!(
                matches!(handled, Err(EventProcessorError::SpecValidation(_))),
                "{handled:?}"
            );
            assert_eq!(edge, None, "a target over the cap is not indexed");
        }
    }
    let (at_cap, _) = normalize_uri(&target(RESOURCE_TAG_TARGET_MAX_LENGTH)).unwrap();
    let (over_cap, _) = normalize_uri(&target(RESOURCE_TAG_TARGET_MAX_LENGTH + 1)).unwrap();
    let lines = authored_since(&author, start).await?;
    assert!(!lines.is_empty());
    assert!(
        lines
            .iter()
            .all(|line| line.resource_id == resource_id(&at_cap)
                && line.resource_id != resource_id(&over_cap)),
        "only the target at the cap is published: {lines:?}"
    );
    test.cleanup_user(&key).await?;
    Ok(())
}

/// Alice and Bob tag the same URL with the same label: their tag files have the same id and
/// path, each in its own folder. Alice's delete publishes her del only, and leaves Bob's edge
/// and the shared Resource: the feed ends put (Alice), put (Bob), del (Alice).
#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_one_taggers_delete_leaves_anothers_tag() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let handler = test.event_processor_runner.event_handler.clone();
    let (alice_key, bob_key) = (Keypair::random(), Keypair::random());
    let alice = test
        .create_user(&alice_key, &test_user("Two taggers Alice", "feed"))
        .await?;
    let bob = test
        .create_user(&bob_key, &test_user("Two taggers Bob", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let target = format!("https://example.com/two-taggers/{alice}");
    let tag = PubkyAppTag::new(target.clone(), "steps".into());
    let path = format!("/pub/pubky.app/tags/{}", tag.create_id());
    let storage: ResourcePath = path.parse()?;
    for (key, author) in [(&alice_key, &alice), (&bob_key, &bob)] {
        test.put(key, &storage, &tag).await?;
        retrieve_and_handle_event_line(&format!("PUT pubky://{author}{path}"), handler.clone())
            .await?;
    }
    test.del(&alice_key, &storage).await?;
    retrieve_and_handle_event_line(&format!("DEL pubky://{alice}{path}"), handler.clone()).await?;

    let mut lines = Vec::new();
    let mut cursor = start;
    loop {
        let (page, _, _) = ResourceChange::get_from_index(cursor, 1000).await?;
        if page.lines.is_empty() {
            break;
        }
        cursor = page.next_cursor;
        lines.extend(
            page.lines
                .into_iter()
                .filter(|line| line.author == alice || line.author == bob),
        );
    }
    // Each PUT is handled twice (by the test's write and by the replay): duplicates
    lines.dedup();
    assert_eq!(
        lines
            .iter()
            .map(|line| (line.op, line.author.as_str()))
            .collect::<Vec<_>>(),
        [
            (ResourceChangeOp::Put, alice.as_str()),
            (ResourceChangeOp::Put, bob.as_str()),
            (ResourceChangeOp::Del, alice.as_str()),
        ],
        "{lines:?}"
    );
    assert!(lines.iter().all(|line| line.object_path == path));
    let (normalized, _) = normalize_uri(&target).unwrap();
    assert_eq!(
        live_references(&lines),
        [(bob.clone(), normalized, "steps".to_string())].into()
    );
    assert_eq!(resource_tag_edge(&alice, &target).await?, None);
    assert_eq!(
        resource_tag_edge(&bob, &target).await?,
        Some(format!("pubky://{bob}{path}")),
        "Bob's edge, and so the Resource, stay"
    );

    test.del(&bob_key, &storage).await?;
    test.cleanup_user(&alice_key).await?;
    test.cleanup_user(&bob_key).await?;
    Ok(())
}

/// The feed writes before the graph on delete: a DEL that stops after appending its del line
/// leaves the edge, and its retry reads the edge again, appends the del again and deletes the
/// edge. A further retry finds no edge and appends nothing.
#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_delete_retry_after_its_append_deletes_the_edge() -> Result<()> {
    use nexus_common::utils::test_utils::fail_feed_dels_after_append;

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Delete retry", "feed"))
        .await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let target = format!("https://example.com/delete-retry/{author}");
    run_tag_steps(&author, &target, &[TagStep::Put]).await?;

    fail_feed_dels_after_append(&author, true);
    let failed = tag_handler(&author, &target, TagStep::Del)?.await;
    fail_feed_dels_after_append(&author, false);
    assert!(failed.is_err(), "the injected failure stops the DEL");
    assert!(
        resource_tag_edge(&author, &target).await?.is_some(),
        "a DEL that stops after its append leaves the edge"
    );
    let ops = |lines: &[ResourceChange]| lines.iter().map(|line| line.op).collect::<Vec<_>>();
    assert_eq!(
        ops(&authored_since(&author, start).await?),
        [ResourceChangeOp::Put, ResourceChangeOp::Del]
    );

    // The retry
    tag_handler(&author, &target, TagStep::Del)?.await?;
    assert_eq!(resource_tag_edge(&author, &target).await?, None);
    // Once more: the edge is gone, nothing is appended
    tag_handler(&author, &target, TagStep::Del)?.await?;
    let lines = authored_since(&author, start).await?;
    assert_eq!(
        ops(&lines),
        [
            ResourceChangeOp::Put,
            ResourceChangeOp::Del,
            ResourceChangeOp::Del
        ]
    );
    assert_eq!(lines[1].uri, lines[2].uri);
    assert!(live_references(&lines).is_empty(), "{lines:?}");
    test.cleanup_user(&key).await?;
    Ok(())
}

/// After puts, deletes, an equivalent file and a second tagger, a reseed rebuilds the feed
/// from the graph with exactly one put per live edge, under the same `(author, uri, label)` the
/// handlers published. Runs alone (`.config/nextest.toml`): it replaces the shared feed.
#[tokio_shared_rt::test(shared)]
async fn test_resource_changes_reseed_gives_one_put_per_edge() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (alice_key, bob_key) = (Keypair::random(), Keypair::random());
    let alice = test
        .create_user(&alice_key, &test_user("Reseed Alice", "feed"))
        .await?;
    let bob = test
        .create_user(&bob_key, &test_user("Reseed Bob", "feed"))
        .await?;
    let shared = format!("https://example.com/reseed/{alice}/shared");
    let gone = format!("https://example.com/reseed/{alice}/gone");
    let twice = format!("https://example.com/reseed/{alice}/twice");
    for author in [&alice, &bob] {
        run_tag_steps(author, &shared, &[TagStep::Put]).await?;
    }
    run_tag_steps(&alice, &gone, &[TagStep::Put, TagStep::Del]).await?;
    run_tag_steps(&alice, &twice, &[TagStep::Put, TagStep::Put]).await?;
    let ours = |line: &ResourceChange| line.author == alice || line.author == bob;
    let (handled, _, _) = ResourceChange::get_from_index(0, usize::MAX).await?;
    let handled: Vec<_> = handled.lines.into_iter().filter(ours).collect();

    ResourceChange::reseed(false).await?;
    let (seeded, _, _) = ResourceChange::get_from_index(0, usize::MAX).await?;
    let seeded: Vec<_> = seeded.lines.into_iter().filter(ours).collect();
    assert!(seeded.iter().all(|line| line.op == ResourceChangeOp::Put));
    let keys: Vec<_> = seeded
        .iter()
        .map(|line| (line.author.clone(), line.uri.clone(), line.label.clone()))
        .collect();
    let distinct: std::collections::BTreeSet<_> = keys.iter().cloned().collect();
    assert_eq!(keys.len(), distinct.len(), "one put per edge: {seeded:?}");
    assert_eq!(live_references(&seeded), live_references(&handled));
    assert_eq!(seeded.len(), 3, "{seeded:?}");

    test.cleanup_user(&alice_key).await?;
    test.cleanup_user(&bob_key).await?;
    Ok(())
}
