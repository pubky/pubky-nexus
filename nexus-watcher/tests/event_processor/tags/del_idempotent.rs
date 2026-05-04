use super::utils::find_post_tag;
use crate::event_processor::posts::utils::find_post_counts;
use crate::event_processor::users::utils::find_user_counts;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::db::OperationOutcome;
use nexus_common::models::post::PostCounts;
use nexus_common::models::tag::post::TagPost;
use nexus_common::models::tag::traits::{TagCollection, TaggersCollection};
use nexus_common::models::user::UserCounts;
use nexus_watcher::events::handlers;
use pubky::Keypair;
use pubky_app_specs::{tag_uri_builder, PubkyAppPost, PubkyAppUser};

/// Use indexed_at far in the past so TAGGED edges don't interfere with
/// hot-tags tests that query this_month/today timeframes.
const OLD_INDEXED_AT: i64 = 1_000_000;

/// Simulate a retry of tag del after a partial failure where Redis cleanup
/// succeeded but graph deletion failed. On retry, counters must NOT be
/// decremented again (guarded by the tagger set membership check).
#[tokio_shared_rt::test(shared)]
async fn test_tag_post_del_retry_no_double_decrement() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create users + post through the watcher (no TAGGED edges)
    let tagger_kp = Keypair::random();
    let tagger = PubkyAppUser {
        bio: Some("test_tag_post_del_retry_no_double_decrement".to_string()),
        image: None,
        links: None,
        name: "Watcher:TagDelRetry:Tagger".to_string(),
        status: None,
    };
    let tagger_id = test.create_user(&tagger_kp, &tagger).await?;

    let author_kp = Keypair::random();
    let author = PubkyAppUser {
        bio: Some("test_tag_post_del_retry_no_double_decrement".to_string()),
        image: None,
        links: None,
        name: "Watcher:TagDelRetry:Author".to_string(),
        status: None,
    };
    let author_id = test.create_user(&author_kp, &author).await?;

    let post = PubkyAppPost {
        content: "Watcher:TagDelRetry:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let (post_id, post_path) = test.create_post(&author_kp, &post).await?;

    // Set up tag state directly via model methods (old timestamp avoids hot-tags interference)
    let label = "retry_idem_test";
    let tag_id = "TEST_TAG_RETRY_01";
    let outcome = TagPost::put_to_graph(
        &tagger_id,
        &author_id,
        Some(&post_id),
        tag_id,
        label,
        OLD_INDEXED_AT,
    )
    .await?;
    assert!(matches!(outcome, OperationOutcome::CreatedOrDeleted));

    // Populate Redis state: tagger set + counters
    TagPost::add_tagger_to_index(&author_id, Some(&post_id), &tagger_id, label).await?;
    UserCounts::increment(&tagger_id, "tagged", None).await?;
    PostCounts::increment_index_field(&[&author_id, &post_id], "tags", None).await?;

    // Verify initial state
    assert_eq!(find_user_counts(&tagger_id).await.tagged, 1);
    assert_eq!(find_post_counts(&author_id, &post_id).await.tags, 1);

    // Simulate partial completion of a previous del attempt:
    // Remove tagger from Redis set + decrement counters (as if Redis ops completed but graph delete failed)
    let tag_post = TagPost(vec![tagger_id.clone()]);
    tag_post
        .del_from_index(&author_id, Some(&post_id), label)
        .await?;
    UserCounts::decrement(&tagger_id, "tagged", None).await?;
    PostCounts::decrement_index_field(&[&author_id, &post_id], "tags", None).await?;

    // Verify simulated state: graph still has TAGGED edge, Redis tagger set empty, counters at 0
    let post_tag = find_post_tag(&author_id, &post_id, label).await?;
    assert!(post_tag.is_some(), "Graph should still have the tag edge");
    assert_eq!(find_user_counts(&tagger_id).await.tagged, 0);
    assert_eq!(find_post_counts(&author_id, &post_id).await.tags, 0);

    // Retry: call del handler directly — should delete graph without double-decrement
    let tag_uri = tag_uri_builder(tagger_id.clone(), tag_id.to_string());
    handlers::tag::del(&tag_uri).await?;

    // Verify final state: graph edge deleted, counters still 0
    let post_tag = find_post_tag(&author_id, &post_id, label).await?;
    assert!(post_tag.is_none(), "Graph tag edge should be deleted");
    assert_eq!(find_user_counts(&tagger_id).await.tagged, 0);
    assert_eq!(find_post_counts(&author_id, &post_id).await.tags, 0);

    // Cleanup
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&tagger_kp).await?;
    test.cleanup_user(&author_kp).await?;

    Ok(())
}

/// After a fully successful delete, a replay of tag del should return Ok
#[tokio_shared_rt::test(shared)]
async fn test_tag_post_del_replay_after_success_skips() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let tagger_kp = Keypair::random();
    let tagger = PubkyAppUser {
        bio: Some("test_tag_post_del_replay_after_success_skips".to_string()),
        image: None,
        links: None,
        name: "Watcher:TagDelReplay:Tagger".to_string(),
        status: None,
    };
    let tagger_id = test.create_user(&tagger_kp, &tagger).await?;

    let author_kp = Keypair::random();
    let author = PubkyAppUser {
        bio: Some("test_tag_post_del_replay_after_success_skips".to_string()),
        image: None,
        links: None,
        name: "Watcher:TagDelReplay:Author".to_string(),
        status: None,
    };
    let author_id = test.create_user(&author_kp, &author).await?;

    let post = PubkyAppPost {
        content: "Watcher:TagDelReplay:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let (post_id, post_path) = test.create_post(&author_kp, &post).await?;

    // Set up tag directly via model, then delete via handler
    let label = "replay_skip_test";
    let tag_id = "TEST_TAG_REPLAY_01";
    TagPost::put_to_graph(
        &tagger_id,
        &author_id,
        Some(&post_id),
        tag_id,
        label,
        OLD_INDEXED_AT,
    )
    .await?;
    TagPost::add_tagger_to_index(&author_id, Some(&post_id), &tagger_id, label).await?;
    UserCounts::increment(&tagger_id, "tagged", None).await?;

    // First delete via handler (should succeed)
    let tag_uri = tag_uri_builder(tagger_id.clone(), tag_id.to_string());
    handlers::tag::del(&tag_uri).await?;

    // Verify fully deleted state
    let post_tag = find_post_tag(&author_id, &post_id, label).await?;
    assert!(post_tag.is_none());

    // Replay: call del again — should succeed as idempotent no-op
    handlers::tag::del(&tag_uri).await?;

    // Cleanup
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&tagger_kp).await?;
    test.cleanup_user(&author_kp).await?;

    Ok(())
}
