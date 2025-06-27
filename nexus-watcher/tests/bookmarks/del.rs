use super::utils::find_post_bookmark;
use crate::users::utils::find_user_counts;
use crate::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::models::post::{Bookmark, PostStream};
use pubky::Keypair;
use pubky_app_specs::{traits::HashId, PubkyAppBookmark, PubkyAppPost, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_unbookmark() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Step 1: Create a user
    let keypair = Keypair::random();
    let bookmarker = PubkyAppUser {
        bio: Some("test_homeserver_unbookmark".to_string()),
        image: None,
        links: None,
        name: "Watcher:Unbookmark:Bookmarker".to_string(),
        status: None,
    };
    let bookmarker_id = test.create_user(&keypair, &bookmarker).await?;

    let author_keypair = Keypair::random();
    let author = PubkyAppUser {
        bio: Some("test_homeserver_unbookmark".to_string()),
        image: None,
        links: None,
        name: "Watcher:Unbookmark:Author".to_string(),
        status: None,
    };
    let author_id = test.create_user(&author_keypair, &author).await?;

    // Step 2: Create a post under that user
    let post = PubkyAppPost {
        content: "Watcher:Unbookmark:Author:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let post_id = test.create_post(&author_id, &post).await?;

    // Step 3: Add a bookmark to the post. Before create a new user
    let bookmark = PubkyAppBookmark {
        uri: format!("pubky://{author_id}/pub/pubky.app/posts/{post_id}"),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let bookmark_id = bookmark.create_id();
    let bookmark_url = format!("pubky://{bookmarker_id}/pub/pubky.app/bookmarks/{bookmark_id}");

    // Put bookmark
    test.put(&bookmark_url, bookmark).await.unwrap();

    // Step 4: Delete bookmark
    test.del(&bookmark_url).await?;

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
    assert!(bookmarks.is_empty(), "The bookmark list should be empty");

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
    test.cleanup_post(&author_id, &post_id).await?;
    test.cleanup_user(&bookmarker_id).await?;
    test.cleanup_user(&author_id).await?;

    Ok(())
}
