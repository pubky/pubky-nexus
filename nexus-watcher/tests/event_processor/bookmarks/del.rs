use super::utils::find_post_bookmark;
use crate::event_processor::utils::watcher::WatcherTest;
use crate::event_processor::{
    users::utils::find_user_counts, utils::watcher::HomeserverHashIdPath,
};
use anyhow::Result;
use nexus_common::models::post::{Bookmark, PostStream};
use pubky::Keypair;
use pubky_app_specs::{post_uri_builder, PubkyAppBookmark, PubkyAppPost, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_unbookmark() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Step 1: Create a user
    let bookmarker_kp = Keypair::random();
    let bookmarker = PubkyAppUser {
        bio: Some("test_homeserver_unbookmark".to_string()),
        image: None,
        links: None,
        name: "Watcher:Unbookmark:Bookmarker".to_string(),
        status: None,
    };
    let bookmarker_id = test.create_user(&bookmarker_kp, &bookmarker).await?;

    let author_kp = Keypair::random();
    let author = PubkyAppUser {
        bio: Some("test_homeserver_unbookmark".to_string()),
        image: None,
        links: None,
        name: "Watcher:Unbookmark:Author".to_string(),
        status: None,
    };
    let author_id = test.create_user(&author_kp, &author).await?;

    // Step 2: Create a post under that user
    let post = PubkyAppPost {
        content: "Watcher:Unbookmark:Author:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let (post_id, post_path) = test.create_post(&author_kp, &post).await?;

    // Step 3: Add a bookmark to the post. Before create a new user
    let bookmark = PubkyAppBookmark {
        uri: post_uri_builder(author_id.clone(), post_id.clone()),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let bookmark_path = bookmark.hs_path();

    // Put bookmark
    test.put(&author_kp, &bookmark_path, bookmark).await?;

    // Step 4: Delete bookmark
    test.del(&author_kp, &bookmark_path).await?;

    // GRAPH_OP: Assert if the event writes the graph
    let result = find_post_bookmark(&author_id, &post_id, &bookmarker_id).await;

    if let Ok(bookmark) = result {
        anyhow::bail!("The bookmark with {:} id still exist", bookmark.id);
    }

    // CACHE_OP: Assert the index is clear of the bookmark
    let bookmarks = PostStream::get_bookmarked_posts(
        &bookmarker_id,
        nexus_common::db::kv::SortOrder::Descending,
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();
    assert!(
        bookmarks.post_keys.is_empty(),
        "The bookmark list should be empty"
    );
    assert!(bookmarks.last_post_score.is_none());

    let exist_bookmark = Bookmark::get_from_index(&author_id, &post_id, &bookmarker_id)
        .await
        .unwrap();
    assert!(
        exist_bookmark.is_none(),
        "The bookmark cannot exist after deletion"
    );

    // Verify bookmark counts have return to 0 for this user
    let user_counts = find_user_counts(&bookmarker_id).await;
    assert_eq!(user_counts.bookmarks, 0);

    // Cleanup user and post
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&bookmarker_kp).await?;
    test.cleanup_user(&author_kp).await?;

    Ok(())
}
