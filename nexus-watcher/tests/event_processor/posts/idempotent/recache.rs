//! A read during a post delete must not cache the post again (#1213), and a
//! replayed DEL must clear a post that is cached but gone from the graph.

use crate::event_processor::posts::utils::{
    check_member_global_timeline_user_post, check_member_post_replies,
    check_member_total_engagement_user_posts, check_member_user_post_timeline,
    check_member_user_replies_timeline, find_post_details, pubky_id, short_post, short_reply,
    test_user,
};
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use deadpool_redis::redis::AsyncCommands;
use nexus_common::db::get_redis_conn;
use nexus_common::models::post::{
    PostCounts, PostDeleting, PostDetails, PostRelationships, POST_DELETING_TTL_SECS,
};
use nexus_common::utils::test_utils::default_ingestor_tests;
use nexus_watcher::errors::EventProcessorError;
use nexus_watcher::events::handlers;
use pubky::Keypair;
use pubky_app_specs::post_uri_builder;

use super::{simulate_partial_del_cleanup_child, simulate_partial_del_cleanup_root, ChildKind};

/// Asserts that nothing of `author_id:post_id` is cached: its three JSON
/// records and its feed entries, including the reply set of `parent`.
async fn assert_post_not_cached(
    author_id: &str,
    post_id: &str,
    parent: Option<(&str, &str)>,
) -> Result<()> {
    let post_key: &[&str] = &[author_id, post_id];
    let ctx = format!("{author_id}:{post_id}");
    assert!(
        PostDetails::get_from_index(author_id, post_id)
            .await?
            .is_none(),
        "PostDetails cached for {ctx}"
    );
    assert!(
        PostCounts::get_from_index(author_id, post_id)
            .await?
            .is_none(),
        "PostCounts cached for {ctx}"
    );
    assert!(
        PostRelationships::get_from_index(author_id, post_id)
            .await?
            .is_none(),
        "PostRelationships cached for {ctx}"
    );
    assert!(
        check_member_global_timeline_user_post(author_id, post_id)
            .await?
            .is_none(),
        "global timeline lists {ctx}"
    );
    assert!(
        check_member_user_post_timeline(author_id, post_id)
            .await?
            .is_none(),
        "author timeline lists {ctx}"
    );
    assert!(
        check_member_user_replies_timeline(author_id, post_id)
            .await?
            .is_none(),
        "author replies lists {ctx}"
    );
    assert!(
        check_member_total_engagement_user_posts(post_key)
            .await?
            .is_none(),
        "engagement set lists {ctx}"
    );
    if let Some((parent_author_id, parent_post_id)) = parent {
        assert!(
            check_member_post_replies(parent_author_id, parent_post_id, post_key)
                .await?
                .is_none(),
            "parent reply set lists {ctx}"
        );
    }
    Ok(())
}

/// Asserts that a finished delete left the marker with its short TTL: set by
/// the delete itself, and shortened after the graph delete.
async fn assert_marker_shortened(author_id: &str, post_id: &str) -> Result<()> {
    let mut conn = get_redis_conn().await?;
    let ttl: i64 = conn.ttl(PostDeleting::key(author_id, post_id)).await?;
    assert!(
        ttl > 0 && ttl <= POST_DELETING_TTL_SECS as i64,
        "marker TTL for {author_id}:{post_id} should be in (0, {POST_DELETING_TTL_SECS}], got {ttl}"
    );
    Ok(())
}

/// Reads all three post models the way `PostView` does, and asserts each one
/// still answers from the graph.
async fn read_post(author_id: &str, post_id: &str) -> Result<()> {
    assert!(PostDetails::get_by_id(author_id, post_id).await?.is_some());
    assert!(PostCounts::get_by_id(author_id, post_id).await?.is_some());
    assert!(PostRelationships::get_by_id(author_id, post_id)
        .await?
        .is_some());
    Ok(())
}

/// A read that misses Redis while a delete is in flight (marker set, Redis
/// cleared, graph node not yet deleted) gets the post from the graph but does
/// not cache it, so nothing is left once the delete finishes.
#[tokio_shared_rt::test(shared)]
async fn test_post_read_during_delete_does_not_recache() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(
            &user_kp,
            &test_user(
                "Watcher:Post:DelRecache:User",
                "test_post_read_during_delete_does_not_recache",
            ),
        )
        .await?;
    let (post_id, _) = test
        .create_post(&user_kp, &short_post("Watcher:Post:DelRecache:Post"))
        .await?;

    // The delete window: marker set, Redis cleared, graph node still there.
    PostDeleting::mark_in_flight(&user_id, &post_id).await?;
    simulate_partial_del_cleanup_root(&user_id, &post_id).await?;

    read_post(&user_id, &post_id).await?;
    assert_post_not_cached(&user_id, &post_id, None).await?;

    // The delete finishes: the graph node goes, and the index stays empty.
    handlers::post::del(
        pubky_id(&user_id)?,
        post_id.clone(),
        &default_ingestor_tests(),
    )
    .await?;
    assert!(find_post_details(&user_id, &post_id).await.is_err());
    assert!(PostDetails::get_by_id(&user_id, &post_id).await?.is_none());
    assert_post_not_cached(&user_id, &post_id, None).await?;
    assert_marker_shortened(&user_id, &post_id).await?;

    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// A plain delete sets the marker itself: once it finishes, the marker holds
/// its short TTL and nothing of the post is cached.
#[tokio_shared_rt::test(shared)]
async fn test_post_del_sets_the_marker() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(
            &user_kp,
            &test_user(
                "Watcher:Post:DelMarker:User",
                "test_post_del_sets_the_marker",
            ),
        )
        .await?;
    let (post_id, post_path) = test
        .create_post(&user_kp, &short_post("Watcher:Post:DelMarker:Post"))
        .await?;

    test.cleanup_post(&user_kp, &post_path).await?;

    assert_marker_shortened(&user_id, &post_id).await?;
    assert!(PostDetails::get_by_id(&user_id, &post_id).await?.is_none());
    assert_post_not_cached(&user_id, &post_id, None).await?;

    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// The same for a reply: the read must not put it back into its parent's reply
/// set or the author's replies.
#[tokio_shared_rt::test(shared)]
async fn test_reply_read_during_delete_does_not_recache() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let alice_kp = Keypair::random();
    let alice_id = test
        .create_user(
            &alice_kp,
            &test_user(
                "Watcher:Post:DelRecacheReply:Alice",
                "test_reply_read_during_delete_does_not_recache",
            ),
        )
        .await?;
    let bob_kp = Keypair::random();
    let bob_id = test
        .create_user(
            &bob_kp,
            &test_user(
                "Watcher:Post:DelRecacheReply:Bob",
                "test_reply_read_during_delete_does_not_recache",
            ),
        )
        .await?;

    let (parent_id, _) = test
        .create_post(
            &alice_kp,
            &short_post("Watcher:Post:DelRecacheReply:Parent"),
        )
        .await?;
    let parent_uri = post_uri_builder(alice_id.clone(), parent_id.clone());
    let (reply_id, _) = test
        .create_post(
            &bob_kp,
            &short_reply("Watcher:Post:DelRecacheReply:Reply", parent_uri),
        )
        .await?;

    PostDeleting::mark_in_flight(&bob_id, &reply_id).await?;
    simulate_partial_del_cleanup_child(&bob_id, &reply_id, &alice_id, &parent_id, ChildKind::Reply)
        .await?;

    read_post(&bob_id, &reply_id).await?;
    assert_post_not_cached(&bob_id, &reply_id, Some((&alice_id, &parent_id))).await?;

    test.cleanup_user(&bob_kp).await?;
    test.cleanup_user(&alice_kp).await?;
    Ok(())
}

/// Without a delete in flight, a read that misses Redis still caches the post.
#[tokio_shared_rt::test(shared)]
async fn test_post_read_repair_still_caches_without_delete() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(
            &user_kp,
            &test_user(
                "Watcher:Post:ReadRepair:User",
                "test_post_read_repair_still_caches_without_delete",
            ),
        )
        .await?;
    let (post_id, _) = test
        .create_post(&user_kp, &short_post("Watcher:Post:ReadRepair:Post"))
        .await?;

    simulate_partial_del_cleanup_root(&user_id, &post_id).await?;
    assert_post_not_cached(&user_id, &post_id, None).await?;

    read_post(&user_id, &post_id).await?;
    assert!(PostDetails::get_from_index(&user_id, &post_id)
        .await?
        .is_some());
    assert!(PostCounts::get_from_index(&user_id, &post_id)
        .await?
        .is_some());
    assert!(PostRelationships::get_from_index(&user_id, &post_id)
        .await?
        .is_some());
    assert!(check_member_global_timeline_user_post(&user_id, &post_id)
        .await?
        .is_some());
    assert!(check_member_user_post_timeline(&user_id, &post_id)
        .await?
        .is_some());

    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// A PUT that recreates a post while the cleanup after its delete runs keeps
/// its cache: the cleanup finds the post back in the graph and rebuilds it.
#[tokio_shared_rt::test(shared)]
async fn test_post_cleanup_keeps_a_recreated_post() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(
            &user_kp,
            &test_user(
                "Watcher:Post:DelRecreate:User",
                "test_post_cleanup_keeps_a_recreated_post",
            ),
        )
        .await?;
    // The recreated post, in the graph and cached, as the PUT left it.
    let (post_id, _) = test
        .create_post(&user_kp, &short_post("Watcher:Post:DelRecreate:Post"))
        .await?;

    handlers::post::clear_post_index(&user_id, &post_id, None).await?;

    assert!(PostDetails::get_from_index(&user_id, &post_id)
        .await?
        .is_some());
    assert!(PostCounts::get_from_index(&user_id, &post_id)
        .await?
        .is_some());
    assert!(PostRelationships::get_from_index(&user_id, &post_id)
        .await?
        .is_some());
    assert!(check_member_global_timeline_user_post(&user_id, &post_id)
        .await?
        .is_some());
    assert!(check_member_user_post_timeline(&user_id, &post_id)
        .await?
        .is_some());

    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// A post cached again after its delete (a zombie from before the marker) is
/// cleared by a replayed DEL, which still reports `SkipIndexing`.
#[tokio_shared_rt::test(shared)]
async fn test_post_del_replay_clears_zombie() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let alice_kp = Keypair::random();
    let alice_id = test
        .create_user(
            &alice_kp,
            &test_user(
                "Watcher:Post:DelZombie:Alice",
                "test_post_del_replay_clears_zombie",
            ),
        )
        .await?;

    let (root_id, root_path) = test
        .create_post(&alice_kp, &short_post("Watcher:Post:DelZombie:Root"))
        .await?;
    let (parent_id, _) = test
        .create_post(&alice_kp, &short_post("Watcher:Post:DelZombie:Parent"))
        .await?;
    let parent_uri = post_uri_builder(alice_id.clone(), parent_id.clone());
    let (reply_id, reply_path) = test
        .create_post(
            &alice_kp,
            &short_reply("Watcher:Post:DelZombie:Reply", parent_uri),
        )
        .await?;

    // Keep the cached copies to write them back after the delete.
    let mut cached = Vec::new();
    for (post_id, parent) in [
        (&root_id, None),
        (&reply_id, Some((alice_id.clone(), parent_id.clone()))),
    ] {
        let details = PostDetails::get_from_index(&alice_id, post_id)
            .await?
            .unwrap();
        let counts = PostCounts::get_from_index(&alice_id, post_id)
            .await?
            .unwrap();
        let relationships = PostRelationships::get_from_index(&alice_id, post_id)
            .await?
            .unwrap();
        cached.push((post_id.clone(), parent, details, counts, relationships));
    }

    test.cleanup_post(&alice_kp, &root_path).await?;
    test.cleanup_post(&alice_kp, &reply_path).await?;

    for (post_id, parent, details, counts, relationships) in &cached {
        // Write the zombie the way the old read path did.
        details
            .put_to_index(&alice_id, parent.clone(), false)
            .await?;
        counts
            .put_to_index(&alice_id, post_id, parent.is_some())
            .await?;
        relationships.put_to_index(&alice_id, post_id).await?;
        assert!(PostDetails::get_by_id(&alice_id, post_id).await?.is_some());

        let result = handlers::post::del(
            pubky_id(&alice_id)?,
            post_id.clone(),
            &default_ingestor_tests(),
        )
        .await;
        assert!(
            matches!(result, Err(EventProcessorError::SkipIndexing)),
            "a DEL of a post gone from the graph should skip, got: {result:?}"
        );

        let parent = parent.as_ref().map(|(a, p)| (a.as_str(), p.as_str()));
        assert_post_not_cached(&alice_id, post_id, parent).await?;
    }

    test.cleanup_user(&alice_kp).await?;
    Ok(())
}
