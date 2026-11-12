//! A read must not index a post again: a read during a post delete would
//! bring the deleted post back (#1213).

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
use nexus_common::models::post::{PostCounts, PostDetails, PostRelationships};
use nexus_common::utils::test_utils::default_ingestor_tests;
use nexus_watcher::events::handlers;
use pubky::Keypair;
use pubky_app_specs::post_uri_builder;

use super::{simulate_partial_del_cleanup_child, simulate_partial_del_cleanup_root, ChildKind};

/// Asserts that `author_id:post_id` is not indexed: no details or
/// relationships record and no feed entries, including the reply set of
/// `parent`. Counts are a TTL'd read-through cache, see [`assert_counts_expire`].
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

/// Asserts that the counts a read cached carry their TTL, so a copy cached
/// during a delete expires on its own.
async fn assert_counts_expire(author_id: &str, post_id: &str) -> Result<()> {
    let mut conn = get_redis_conn().await?;
    let ttl: i64 = conn
        .ttl(format!("Post:Counts:{author_id}:{post_id}"))
        .await?;
    assert!(
        ttl > 0 && ttl <= 600,
        "PostCounts TTL for {author_id}:{post_id} should be in (0, 600], got {ttl}"
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

/// A read that misses Redis during a post delete (Redis cleared, graph node
/// not yet deleted) gets the post from the graph but does not index it again,
/// so nothing but its expiring counts is left once the delete finishes.
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

    // The delete window: Redis cleared, graph node still there.
    simulate_partial_del_cleanup_root(&user_id, &post_id).await?;

    read_post(&user_id, &post_id).await?;
    assert_post_not_cached(&user_id, &post_id, None).await?;
    assert_counts_expire(&user_id, &post_id).await?;

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

    simulate_partial_del_cleanup_child(&bob_id, &reply_id, &alice_id, &parent_id, ChildKind::Reply)
        .await?;

    read_post(&bob_id, &reply_id).await?;
    assert_post_not_cached(&bob_id, &reply_id, Some((&alice_id, &parent_id))).await?;

    test.cleanup_user(&bob_kp).await?;
    test.cleanup_user(&alice_kp).await?;
    Ok(())
}
