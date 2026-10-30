use super::utils::bookmarked_post_keys;
use crate::event_processor::{
    users::utils::find_user_counts, utils::watcher::HomeserverHashIdPath,
    utils::watcher::WatcherTest,
};
use anyhow::Result;
use nexus_common::models::post::Bookmark;
use pubky::Keypair;
use pubky_app_specs::{post_uri_builder, PubkyAppBookmark, PubkyAppPost, PubkyAppUser};

/// An unbookmark is gone from the point read and the stream at once.
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_unbookmark() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

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
        lock: None,
    };
    let (post_id, post_path) = test.create_post(&author_kp, &post).await?;

    // Step 3: The bookmarker bookmarks the author's post
    let bookmark = PubkyAppBookmark {
        uri: post_uri_builder(author_id.clone(), post_id.clone()),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let bookmark_path = bookmark.hs_path();

    test.put(&bookmarker_kp, &bookmark_path, bookmark).await?;

    assert_eq!(
        bookmarked_post_keys(&bookmarker_id).await?,
        vec![format!("{author_id}:{post_id}")]
    );
    assert!(
        Bookmark::get_by_id(&author_id, &post_id, Some(&bookmarker_id))
            .await?
            .is_some(),
        "The bookmark has to be served before the delete"
    );

    // Step 4: Delete bookmark
    test.del(&bookmarker_kp, &bookmark_path).await?;

    assert!(
        Bookmark::get_by_id(&author_id, &post_id, Some(&bookmarker_id))
            .await?
            .is_none(),
        "The point read must not serve the bookmark"
    );
    assert!(
        bookmarked_post_keys(&bookmarker_id).await?.is_empty(),
        "The bookmark list should be empty"
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
