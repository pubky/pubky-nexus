use super::utils::{
    bookmarked_post_keys, delete_retired_keys, find_post_bookmark, read_retired_keys,
    seed_retired_keys,
};
use crate::event_processor::{
    users::utils::find_user_counts, utils::watcher::HomeserverHashIdPath,
    utils::watcher::WatcherTest,
};
use anyhow::Result;
use nexus_common::models::post::Bookmark;
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder, traits::HashId, PubkyAppBookmark, PubkyAppPost, PubkyAppUser,
};

/// An unbookmark is gone from the point read and the stream at once, even with
/// copies of the bookmark left in the retired Redis keys, and the watcher
/// leaves those copies alone.
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
    let bookmark_id = bookmark.create_id();
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

    // Copies an older deployment left in Redis
    seed_retired_keys(&author_id, &post_id, &bookmarker_id, &bookmark_id).await?;
    let seeded = read_retired_keys(&author_id, &post_id, &bookmarker_id).await?;

    // Step 4: Delete bookmark
    test.del(&bookmarker_kp, &bookmark_path).await?;

    // Read everything before asserting, so the copies are always removed
    let edge = find_post_bookmark(&author_id, &post_id, &bookmarker_id).await;
    let served = Bookmark::get_by_id(&author_id, &post_id, Some(&bookmarker_id)).await?;
    let stream = bookmarked_post_keys(&bookmarker_id).await?;
    let left = read_retired_keys(&author_id, &post_id, &bookmarker_id).await?;
    delete_retired_keys(&author_id, &post_id, &bookmarker_id).await?;

    // GRAPH_OP: Assert if the event writes the graph
    assert!(edge.is_err(), "The bookmark cannot exist after deletion");
    assert!(served.is_none(), "The point read must not serve the copy");
    assert!(stream.is_empty(), "The stream must not serve the copy");
    assert_eq!(left, seeded, "The watcher must not touch the retired keys");

    // Verify bookmark counts have return to 0 for this user
    let user_counts = find_user_counts(&bookmarker_id).await;
    assert_eq!(user_counts.bookmarks, 0);

    // Cleanup user and post
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&bookmarker_kp).await?;
    test.cleanup_user(&author_kp).await?;

    Ok(())
}
