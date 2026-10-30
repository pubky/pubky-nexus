use super::utils::{bookmark_of, bookmarked_post_keys};
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use nexus_common::models::post::{Bookmark, PostStream};
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder, traits::HashId, PubkyAppBookmark, PubkyAppPost, PubkyAppUser,
};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_viewer_bookmark() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    // Step 1: Create a user
    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_homeserver_viewer_bookmark".to_string()),
        image: None,
        links: None,
        name: "Watcher:ViewerBookmark:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    // Step 2: Create a post under that user
    let post = PubkyAppPost {
        content: "Watcher:ViewerBookmark:User:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
        lock: None,
    };
    let (post_id, post_path) = test.create_post(&user_kp, &post).await?;

    // Step 3: Add a bookmark to the post. Before create a new user
    let viewer_kp = Keypair::random();

    let viewer_user = PubkyAppUser {
        bio: Some("test_homeserver_viewer_bookmark".to_string()),
        image: None,
        links: None,
        name: "Watcher:ViewerBookmark:Viewer".to_string(),
        status: None,
    };
    let viewer_id = test.create_user(&viewer_kp, &viewer_user).await?;

    let bookmark = PubkyAppBookmark {
        uri: post_uri_builder(user_id.clone(), post_id.clone()),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let bookmark_path = bookmark.hs_path();
    let bookmark_id = bookmark.create_id();

    // Put bookmark
    test.put(&viewer_kp, &bookmark_path, bookmark)
        .await
        .unwrap();

    // Step 4: Verify the bookmark exists in Nexus
    // GRAPH_OP: Assert if the event writes the graph
    let viewer_bookmark = Bookmark::get_by_id(&user_id, &post_id, Some(&viewer_id))
        .await?
        .expect("The bookmark has to be served");
    assert_eq!(viewer_bookmark.id, bookmark_id);

    // The bookmark is served in the viewer's stream
    let result_bookmarks = PostStream::get_bookmarked_posts(
        &viewer_id,
        nexus_common::db::kv::SortOrder::Descending,
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();

    assert_eq!(result_bookmarks.post_keys.len(), 1);
    assert_eq!(
        result_bookmarks.post_keys[0],
        format!("{user_id}:{post_id}")
    );
    assert!(result_bookmarks.last_post_score.is_some());

    // Cleanup user and post
    test.cleanup_post(&user_kp, &post_path).await?;
    test.cleanup_user(&user_kp).await?;
    test.cleanup_user(&viewer_kp).await?;

    Ok(())
}

/// Each viewer sees only their own bookmarks. Bookmarks of one post share an
/// id across users, so one viewer's unbookmark must leave the other's alone.
#[tokio_shared_rt::test(shared)]
async fn test_bookmarks_are_isolated_per_viewer() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author = PubkyAppUser {
        bio: Some("test_bookmarks_are_isolated_per_viewer".to_string()),
        image: None,
        links: None,
        name: "Watcher:BookmarkIsolation:Author".to_string(),
        status: None,
    };
    let author_id = test.create_user(&author_kp, &author).await?;

    let viewer_a_kp = Keypair::random();
    let viewer_a = PubkyAppUser {
        name: "Watcher:BookmarkIsolation:ViewerA".to_string(),
        ..author.clone()
    };
    let viewer_a_id = test.create_user(&viewer_a_kp, &viewer_a).await?;

    let viewer_b_kp = Keypair::random();
    let viewer_b = PubkyAppUser {
        name: "Watcher:BookmarkIsolation:ViewerB".to_string(),
        ..author.clone()
    };
    let viewer_b_id = test.create_user(&viewer_b_kp, &viewer_b).await?;

    let post = PubkyAppPost {
        content: "Watcher:BookmarkIsolation:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
        lock: None,
    };
    let (shared_id, shared_path) = test.create_post(&author_kp, &post).await?;
    let (only_b_id, only_b_path) = test.create_post(&author_kp, &post).await?;
    let shared_key = format!("{author_id}:{shared_id}");
    let only_b_key = format!("{author_id}:{only_b_id}");

    // A bookmarks the shared post; B bookmarks it too, and one more
    let shared_bookmark = bookmark_of(&author_id, &shared_id);
    let shared_bookmark_path = shared_bookmark.hs_path();
    test.put(&viewer_a_kp, &shared_bookmark_path, &shared_bookmark)
        .await?;
    test.put(&viewer_b_kp, &shared_bookmark_path, &shared_bookmark)
        .await?;
    let only_b_bookmark = bookmark_of(&author_id, &only_b_id);
    test.put(&viewer_b_kp, &only_b_bookmark.hs_path(), &only_b_bookmark)
        .await?;

    let mut b_keys = bookmarked_post_keys(&viewer_b_id).await?;
    b_keys.sort();
    let mut expected_b_keys = vec![shared_key.clone(), only_b_key.clone()];
    expected_b_keys.sort();
    assert_eq!(bookmarked_post_keys(&viewer_a_id).await?, vec![shared_key]);
    assert_eq!(b_keys, expected_b_keys);
    assert!(
        Bookmark::get_by_id(&author_id, &only_b_id, Some(&viewer_a_id))
            .await?
            .is_none(),
        "A must not see B's bookmark"
    );

    // A unbookmarks the shared post
    test.del(&viewer_a_kp, &shared_bookmark_path).await?;

    assert!(bookmarked_post_keys(&viewer_a_id).await?.is_empty());
    assert!(
        Bookmark::get_by_id(&author_id, &shared_id, Some(&viewer_a_id))
            .await?
            .is_none()
    );
    let mut b_keys = bookmarked_post_keys(&viewer_b_id).await?;
    b_keys.sort();
    assert_eq!(b_keys, expected_b_keys, "B keeps both bookmarks");
    let b_shared = Bookmark::get_by_id(&author_id, &shared_id, Some(&viewer_b_id))
        .await?
        .expect("B keeps the shared bookmark");
    assert_eq!(b_shared.id, shared_bookmark.create_id());

    test.cleanup_post(&author_kp, &shared_path).await?;
    test.cleanup_post(&author_kp, &only_b_path).await?;
    test.cleanup_user(&viewer_a_kp).await?;
    test.cleanup_user(&viewer_b_kp).await?;
    test.cleanup_user(&author_kp).await?;

    Ok(())
}
