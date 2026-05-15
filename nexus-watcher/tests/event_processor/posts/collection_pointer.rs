//! Watcher integration tests for `PubkyAppCollectionPointer` ingestion.
//!
//! The spec primitive (path `/pub/pubky.app/collections/<owner>/<post_id>`,
//! body `{ created_at: i64 }`) is unified, but the watcher applies role
//! inference at index time:
//!
//! - **own-pointer** (URI host == path owner) → no-op. The homeserver state
//!   is the sovereign index; Nexus does nothing.
//! - **follow-pointer** (URI host != path owner) → MERGE the
//!   `(:User)-[:FOLLOWS_COLLECTION]->(:Post {kind:'collection'})` edge.
//!
//! Notification emission lands in a follow-on commit (Phase 2 commit 3).

use anyhow::Result;
use chrono::Utc;
use nexus_common::db::{fetch_key_from_graph, graph::Query};
use nexus_common::models::notification::{Notification, NotificationBody};
use nexus_common::types::Pagination;
use nexus_watcher::events::handlers::collection_pointer;
use pubky::{Keypair, ResourcePath};
use pubky_app_specs::{PubkyAppCollectionPointer, PubkyId};

use super::utils::{collection_post, test_user};
use crate::event_processor::utils::watcher::WatcherTest;

/// Returns `Some(true)` iff a `:FOLLOWS_COLLECTION` edge exists from
/// `follower_id` to a `:Post {id: target_post_id}`.
async fn follow_collection_edge_exists(
    follower_id: &str,
    target_post_id: &str,
) -> Result<Option<bool>> {
    let query = Query::new(
        "test_find_follow_collection_edge",
        "MATCH (:User {id: $follower_id})-[:FOLLOWS_COLLECTION]->(:Post {id: $target_post_id})
         RETURN true AS exists",
    )
    .param("follower_id", follower_id.to_string())
    .param("target_post_id", target_post_id.to_string());
    Ok(fetch_key_from_graph(query, "exists").await.unwrap_or(None))
}

fn pointer_path(owner_id: &str, post_id: &str) -> ResourcePath {
    PubkyAppCollectionPointer::create_path(owner_id, post_id)
        .parse()
        .unwrap()
}

fn pointer_body() -> PubkyAppCollectionPointer {
    PubkyAppCollectionPointer {
        created_at: Utc::now().timestamp_millis(),
    }
}

/// Returns the number of `FollowCollection` notifications for `recipient_id`
/// that reference the (`expected_follower`, `expected_post_id`) pair.
async fn count_follow_collection_notifications(
    recipient_id: &str,
    expected_follower: &str,
    expected_post_id: &str,
) -> Result<usize> {
    let notifs = Notification::get_by_id(recipient_id, Pagination::default()).await?;
    Ok(notifs
        .iter()
        .filter(|n| match &n.body {
            NotificationBody::FollowCollection {
                followed_by,
                collection_post_id,
                ..
            } => followed_by == expected_follower && collection_post_id == expected_post_id,
            _ => false,
        })
        .count())
}

/// Own-pointer (URI host == path owner): Nexus must do nothing. No graph
/// edge, no notification, no error.
#[tokio_shared_rt::test(shared)]
async fn test_put_own_pointer_is_noop() -> Result<()> {
    let mut test = WatcherTest::setup().await?;
    let user_kp = Keypair::random();
    let user = test_user("Watcher:Pointer:OwnNoop", "pointer_own_noop");
    let user_id = test.create_user(&user_kp, &user).await?;

    // Author the collection so the owner / post_id pair points at a real node.
    let collection = collection_post("My favorites", None, vec![]);
    let (post_id, _post_path) = test.create_post(&user_kp, &collection).await?;

    // PUT an own-pointer (owner == follower).
    let path = pointer_path(&user_id, &post_id);
    test.put(&user_kp, &path, pointer_body()).await?;

    // Assertion: no :FOLLOWS_COLLECTION edge.
    assert!(
        follow_collection_edge_exists(&user_id, &post_id)
            .await?
            .is_none(),
        "own-pointer must NOT create a :FOLLOWS_COLLECTION edge"
    );

    // Assertion: no self-notification.
    assert_eq!(
        count_follow_collection_notifications(&user_id, &user_id, &post_id).await?,
        0,
        "own-pointer must NOT emit a self-notification"
    );

    test.del(&user_kp, &path).await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// Follow-pointer (URI host != path owner): MERGE the
/// `:FOLLOWS_COLLECTION` edge AND emit a `FollowCollection` notification
/// to the target owner.
#[tokio_shared_rt::test(shared)]
async fn test_put_follow_pointer_creates_edge_and_notifies() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Author (target collection's owner).
    let author_kp = Keypair::random();
    let author = test_user("Watcher:Pointer:Author", "pointer_author");
    let author_id = test.create_user(&author_kp, &author).await?;
    let collection = collection_post("Author's reads", None, vec![]);
    let (post_id, post_path) = test.create_post(&author_kp, &collection).await?;

    // Follower (different user).
    let follower_kp = Keypair::random();
    let follower = test_user("Watcher:Pointer:Follower", "pointer_follower");
    let follower_id = test.create_user(&follower_kp, &follower).await?;

    // Follow-pointer PUT.
    let path = pointer_path(&author_id, &post_id);
    test.put(&follower_kp, &path, pointer_body()).await?;

    assert_eq!(
        follow_collection_edge_exists(&follower_id, &post_id).await?,
        Some(true),
        "follow-pointer must create a :FOLLOWS_COLLECTION edge"
    );

    // Notification: author received exactly one FollowCollection from follower.
    assert_eq!(
        count_follow_collection_notifications(&author_id, &follower_id, &post_id).await?,
        1,
        "target owner must receive exactly one FollowCollection notification"
    );

    // DEL: edge removed, but no new notification fires.
    test.del(&follower_kp, &path).await?;
    assert_eq!(
        count_follow_collection_notifications(&author_id, &follower_id, &post_id).await?,
        1,
        "DEL must NOT emit a new notification (the original PUT notification stays)"
    );

    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&author_kp).await?;
    test.cleanup_user(&follower_kp).await?;
    Ok(())
}

/// Re-PUT of an existing follow-pointer is idempotent (graph MERGE returns
/// `Updated`; handler skips notification re-fire).
#[tokio_shared_rt::test(shared)]
async fn test_put_follow_pointer_idempotent() -> Result<()> {
    let mut test = WatcherTest::setup().await?;
    let author_kp = Keypair::random();
    let author = test_user("Watcher:Pointer:IdempotAuthor", "pointer_idempot_author");
    let author_id = test.create_user(&author_kp, &author).await?;
    let collection = collection_post("Idempot", None, vec![]);
    let (post_id, post_path) = test.create_post(&author_kp, &collection).await?;

    let follower_kp = Keypair::random();
    let follower = test_user(
        "Watcher:Pointer:IdempotFollower",
        "pointer_idempot_follower",
    );
    let follower_id = test.create_user(&follower_kp, &follower).await?;

    let path = pointer_path(&author_id, &post_id);
    // First PUT: edge created + 1 notification.
    test.put(&follower_kp, &path, pointer_body()).await?;
    // Second PUT: idempotent; edge still exists; NO duplicate notification.
    test.put(&follower_kp, &path, pointer_body()).await?;

    assert_eq!(
        follow_collection_edge_exists(&follower_id, &post_id).await?,
        Some(true),
        "re-PUT must not break the edge"
    );
    assert_eq!(
        count_follow_collection_notifications(&author_id, &follower_id, &post_id).await?,
        1,
        "re-PUT must NOT duplicate the FollowCollection notification"
    );

    test.del(&follower_kp, &path).await?;
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&author_kp).await?;
    test.cleanup_user(&follower_kp).await?;
    Ok(())
}

/// Follow-pointer to a non-Collection post: the Cypher MATCH on
/// `kind: 'collection'` fails, so no edge is created. The watcher returns
/// MissingDependency (retryable), but the eventual outcome is no edge.
#[tokio_shared_rt::test(shared)]
async fn test_put_follow_pointer_non_collection_target_skips_indexing() -> Result<()> {
    let mut test = WatcherTest::setup().await?;
    let author_kp = Keypair::random();
    let author = test_user("Watcher:Pointer:NonColAuthor", "pointer_noncol_author");
    let author_id = test.create_user(&author_kp, &author).await?;

    // Author a kind=Short post (the default kind), NOT a Collection.
    let short = pubky_app_specs::PubkyAppPost {
        content: "just a short".to_string(),
        kind: pubky_app_specs::PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let (post_id, post_path) = test.create_post(&author_kp, &short).await?;

    let follower_kp = Keypair::random();
    let follower = test_user("Watcher:Pointer:NonColFollower", "pointer_noncol_follower");
    let follower_id = test.create_user(&follower_kp, &follower).await?;

    let path = pointer_path(&author_id, &post_id);
    // PUT may succeed at the homeserver but the watcher will fail to MERGE
    // the edge (Cypher MATCH `kind: 'collection'` yields zero rows → the
    // handler returns MissingDependency, which the processor enqueues for
    // retry). The test runner's `ensure_event_processing_complete` may
    // either swallow the retry-queue enqueue or surface an error — we don't
    // care which; we care that NO edge exists.
    let _ = test.put(&follower_kp, &path, pointer_body()).await;

    assert!(
        follow_collection_edge_exists(&follower_id, &post_id)
            .await?
            .is_none(),
        "follow-pointer to a non-Collection post must NOT create a :FOLLOWS_COLLECTION edge"
    );

    let _ = test.del(&follower_kp, &path).await;
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&author_kp).await?;
    test.cleanup_user(&follower_kp).await?;
    Ok(())
}

/// DEL of an existing follow-pointer removes the graph edge.
#[tokio_shared_rt::test(shared)]
async fn test_del_follow_pointer_removes_edge() -> Result<()> {
    let mut test = WatcherTest::setup().await?;
    let author_kp = Keypair::random();
    let author = test_user("Watcher:Pointer:DelAuthor", "pointer_del_author");
    let author_id = test.create_user(&author_kp, &author).await?;
    let collection = collection_post("Del", None, vec![]);
    let (post_id, post_path) = test.create_post(&author_kp, &collection).await?;

    let follower_kp = Keypair::random();
    let follower = test_user("Watcher:Pointer:DelFollower", "pointer_del_follower");
    let follower_id = test.create_user(&follower_kp, &follower).await?;

    let path = pointer_path(&author_id, &post_id);
    test.put(&follower_kp, &path, pointer_body()).await?;
    assert_eq!(
        follow_collection_edge_exists(&follower_id, &post_id).await?,
        Some(true),
    );

    // DEL the follow-pointer.
    test.del(&follower_kp, &path).await?;

    assert!(
        follow_collection_edge_exists(&follower_id, &post_id)
            .await?
            .is_none(),
        "DEL must remove the :FOLLOWS_COLLECTION edge"
    );

    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&author_kp).await?;
    test.cleanup_user(&follower_kp).await?;
    Ok(())
}

/// Calling `sync_del` directly twice (simulating a watcher retry after the
/// homeserver-side resource is already gone) must not error or corrupt
/// state. We bypass `test.del` here because the homeserver itself rejects
/// DEL on non-existent paths with 404 — that's an upstream-layer concern,
/// not the watcher's idempotency invariant.
#[tokio_shared_rt::test(shared)]
async fn test_sync_del_follow_pointer_idempotent() -> Result<()> {
    let mut test = WatcherTest::setup().await?;
    let author_kp = Keypair::random();
    let author = test_user("Watcher:Pointer:DelIdAuthor", "pointer_del_id_author");
    let _author_id = test.create_user(&author_kp, &author).await?;
    let collection = collection_post("DelId", None, vec![]);
    let (post_id, post_path) = test.create_post(&author_kp, &collection).await?;

    let follower_kp = Keypair::random();
    let follower = test_user("Watcher:Pointer:DelIdFollower", "pointer_del_id_follower");
    let _follower_id = test.create_user(&follower_kp, &follower).await?;

    let follower_pubky = PubkyId::from(follower_kp.clone());
    let author_pubky = PubkyId::from(author_kp.clone());

    // First call — no edge to delete; must succeed.
    collection_pointer::sync_del(
        follower_pubky.clone(),
        author_pubky.clone(),
        post_id.clone(),
    )
    .await?;

    // Second call — still no edge; must succeed (retry idempotency).
    collection_pointer::sync_del(follower_pubky, author_pubky, post_id).await?;

    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&author_kp).await?;
    test.cleanup_user(&follower_kp).await?;
    Ok(())
}
