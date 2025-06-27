use super::utils::find_post_bookmark;
use crate::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::models::post::PostStream;
use pubky::Keypair;
use pubky_app_specs::{traits::HashId, PubkyAppBookmark, PubkyAppPost, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_viewer_bookmark() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Step 1: Create a user
    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_homeserver_viewer_bookmark".to_string()),
        image: None,
        links: None,
        name: "Watcher:ViewerBookmark:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    // Step 2: Create a post under that user
    let post = PubkyAppPost {
        content: "Watcher:ViewerBookmark:User:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let post_id = test.create_post(&user_id, &post).await?;

    // Step 3: Add a bookmark to the post. Before create a new user
    let viewer_keypair = Keypair::random();

    let viewer_user = PubkyAppUser {
        bio: Some("test_homeserver_viewer_bookmark".to_string()),
        image: None,
        links: None,
        name: "Watcher:ViewerBookmark:Viewer".to_string(),
        status: None,
    };
    let viewer_id = test.create_user(&viewer_keypair, &viewer_user).await?;

    let bookmark = PubkyAppBookmark {
        uri: format!("pubky://{user_id}/pub/pubky.app/posts/{post_id}"),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let bookmark_id = bookmark.create_id();
    let bookmark_url = format!("pubky://{viewer_id}/pub/pubky.app/bookmarks/{bookmark_id}");

    // Put bookmark
    test.put(&bookmark_url, bookmark).await.unwrap();

    // Step 4: Verify the bookmark exists in Nexus
    // GRAPH_OP: Assert if the event writes the graph
    let viewer_bookmark = find_post_bookmark(&user_id, &post_id, &viewer_id)
        .await
        .unwrap();
    assert_eq!(viewer_bookmark.id, bookmark_id);

    // INDEX_OP: Assert if the event writes the indexes
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

    assert_eq!(result_bookmarks.len(), 1);
    assert_eq!(result_bookmarks[0], format!("{user_id}:{post_id}"));

    // Cleanup user and post
    test.cleanup_post(&user_id, &post_id).await?;
    test.cleanup_user(&user_id).await?;
    test.cleanup_user(&viewer_id).await?;

    Ok(())
}
