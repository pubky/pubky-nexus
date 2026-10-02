use crate::event_processor::posts::utils::{
    find_post_details, pubky_id, short_post, short_repost, test_user,
};
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::notification::{Notification, NotificationBody, PostChangedSource};
use nexus_common::models::post::{PostCounts, PostDetails, PostView};
use nexus_common::types::Pagination;
use nexus_common::utils::test_utils::default_ingestor_tests;
use nexus_watcher::events::handlers;
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder, PubkyAppBookmark, PubkyAppPost, PubkyAppPostKind, PubkyAppTag, PubkyAppUser,
};

#[tokio_shared_rt::test(shared)]
async fn test_delete_post_with_relationships() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    // Create a new user
    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("Test user for post deletion".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostDelete:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    // Create a post without any relationships
    let post = PubkyAppPost {
        content: "User's post to be deleted".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
        lock: None,
    };
    let (post_id, post_path) = test.create_post(&user_kp, &post).await?;

    // Create a tag
    let tag = PubkyAppTag {
        uri: post_uri_builder(user_id.clone(), post_id.clone()),
        label: "funny".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();

    // Put tag
    test.put(&user_kp, &tag_path, tag).await?;

    // Delete the post using the event handler
    test.cleanup_post(&user_kp, &post_path).await?;

    // Post details should still exist, cleared and flagged as deleted
    let post_details_result = PostDetails::get_by_id(&user_id, &post_id)
        .await
        .unwrap()
        .expect("Post details still be found after deletion");
    assert!(
        post_details_result.deleted,
        "Post should be marked as deleted after deletion with relationships"
    );
    assert_eq!(
        post_details_result.content, "",
        "Post content should be cleared after deletion"
    );

    // The graph holds the same tombstone
    let graph_details = find_post_details(&user_id, &post_id).await?;
    assert!(
        graph_details.deleted,
        "Graph node should carry deleted = true"
    );
    assert_eq!(graph_details.content, "", "Graph content should be cleared");

    // Attempt to find post counts; should not exist
    let post_counts_result = PostCounts::get_by_id(&user_id, &post_id).await.unwrap();
    assert!(
        post_counts_result.is_some(),
        "Post counts should exist after deletion"
    );

    // Attempt to get post view; should not exist
    let post_view = PostView::get_by_id(&user_id, &post_id, None, None, None)
        .await
        .unwrap();
    assert!(post_view.is_some(), "Post view should exist after deletion");

    Ok(())
}

/// A repost with no content or attachments differs from its own tombstone only
/// in the deleted flag. Deleting it must still flag it and tell its bookmarker
/// that it was deleted.
#[tokio_shared_rt::test(shared)]
async fn test_delete_empty_repost_with_relationships() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:PostDelete:EmptyRepost:Author", "author"),
        )
        .await?;
    let (post_id, _) = test
        .create_post(
            &author_kp,
            &short_post("Watcher:PostDelete:EmptyRepost:Post"),
        )
        .await?;

    let reposter_kp = Keypair::random();
    let reposter_id = test
        .create_user(
            &reposter_kp,
            &test_user("Watcher:PostDelete:EmptyRepost:Reposter", "reposter"),
        )
        .await?;
    let repost = short_repost("", post_uri_builder(author_id.clone(), post_id.clone()));
    let (repost_id, repost_path) = test.create_post(&reposter_kp, &repost).await?;

    // The bookmark keeps the repost from being hard-deleted
    let bookmarker_kp = Keypair::random();
    let bookmarker_id = test
        .create_user(
            &bookmarker_kp,
            &test_user("Watcher:PostDelete:EmptyRepost:Bookmarker", "bookmarker"),
        )
        .await?;
    let bookmark = PubkyAppBookmark {
        uri: post_uri_builder(reposter_id.clone(), repost_id.clone()),
        created_at: Utc::now().timestamp_millis(),
    };
    test.put(&bookmarker_kp, &bookmark.hs_path(), bookmark)
        .await?;

    test.cleanup_post(&reposter_kp, &repost_path).await?;

    let details = PostDetails::get_by_id(&reposter_id, &repost_id)
        .await?
        .expect("the bookmarked repost should survive as a tombstone");
    assert!(details.deleted, "Cached repost should be flagged deleted");
    assert!(
        find_post_details(&reposter_id, &repost_id).await?.deleted,
        "Graph repost should be flagged deleted"
    );

    let notifications = Notification::get_by_id(&bookmarker_id, Pagination::default()).await?;
    assert_eq!(
        notifications.len(),
        1,
        "The bookmarker should have exactly one notification"
    );
    assert!(
        matches!(
            notifications[0].body,
            NotificationBody::PostDeleted {
                delete_source: PostChangedSource::Bookmark,
                ..
            }
        ),
        "Expected a PostDeleted bookmark notification, got {:?}",
        notifications[0].body
    );

    Ok(())
}

/// Resurrection: delete a post with relationships, re-PUT it at the same path,
/// and assert deleted == false with the new content.
#[tokio_shared_rt::test(shared)]
async fn test_post_resurrection() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:PostResurrection:Author", "author"),
        )
        .await?;
    let (post_id, post_path) = test
        .create_post(&author_kp, &short_post("Watcher:PostResurrection:Post"))
        .await?;

    // A tag from another user keeps the node on deletion
    let tagger_kp = Keypair::random();
    test.create_user(
        &tagger_kp,
        &test_user("Watcher:PostResurrection:Tagger", "tagger"),
    )
    .await?;
    let tag = PubkyAppTag {
        uri: post_uri_builder(author_id.clone(), post_id.clone()),
        label: "resurrection".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    test.put(&tagger_kp, &tag.hs_path(), tag).await?;

    test.cleanup_post(&author_kp, &post_path).await?;
    assert!(
        find_post_details(&author_id, &post_id).await?.deleted,
        "Post should be marked as deleted after tombstoning"
    );

    // Re-PUT the post at the same path
    let revived = short_post("Watcher:PostResurrection:Revived");
    test.put(&author_kp, &post_path, &revived).await?;

    // create_post always writes the flag, so a re-PUT clears it
    let details = PostDetails::get_by_id(&author_id, &post_id)
        .await?
        .expect("the resurrected post should be indexed");
    assert!(
        !details.deleted,
        "Post should NOT be deleted after re-PUTting it (resurrection)"
    );
    assert_eq!(details.content, revived.content);
    assert!(
        !find_post_details(&author_id, &post_id).await?.deleted,
        "Graph node should be live again"
    );

    Ok(())
}

/// Regression: a post whose content is the literal "[DELETED]" must report
/// deleted: false, and editing a post to that content is an edit, not a
/// deletion. Deletion is the flag, never the content.
///
/// `PubkyAppPost::validate` rejects that content, so the shape is unreachable
/// through a post event and only exists for rows written before the flag. The
/// test reproduces it by calling `post::sync_put` directly, below the
/// validation boundary.
#[tokio_shared_rt::test(shared)]
async fn test_live_post_with_sentinel_content_is_not_tombstoned() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:PostSentinelContent:Author", "author"),
        )
        .await?;
    let (post_id, _) = test
        .create_post(&author_kp, &short_post("Watcher:PostSentinelContent:Post"))
        .await?;

    // A bookmark gives the edit someone to notify
    let bookmarker_kp = Keypair::random();
    let bookmarker_id = test
        .create_user(
            &bookmarker_kp,
            &test_user("Watcher:PostSentinelContent:Bookmarker", "bookmarker"),
        )
        .await?;
    let bookmark = PubkyAppBookmark {
        uri: post_uri_builder(author_id.clone(), post_id.clone()),
        created_at: Utc::now().timestamp_millis(),
    };
    test.put(&bookmarker_kp, &bookmark.hs_path(), bookmark)
        .await?;

    // Edit to the legacy sentinel through the real handler, so the graph and
    // the cache are written in production order
    handlers::post::sync_put(
        short_post("[DELETED]"),
        pubky_id(&author_id)?,
        post_id.clone(),
        &default_ingestor_tests(),
    )
    .await?;

    let details = PostDetails::get_by_id(&author_id, &post_id)
        .await?
        .expect("the edited post should be indexed");
    assert_eq!(details.content, "[DELETED]");
    assert!(
        !details.deleted,
        "A live post with content '[DELETED]' should NOT be treated as deleted"
    );
    assert!(!find_post_details(&author_id, &post_id).await?.deleted);

    // The bookmarker is told about an edit, not a deletion
    let notifications = Notification::get_by_id(&bookmarker_id, Pagination::default()).await?;
    assert_eq!(
        notifications.len(),
        1,
        "The bookmarker should have exactly one notification"
    );
    assert!(
        matches!(notifications[0].body, NotificationBody::PostEdited { .. }),
        "Expected a PostEdited notification, got {:?}",
        notifications[0].body
    );

    Ok(())
}
