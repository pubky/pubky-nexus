use super::utils::find_post_bookmark;
use crate::watcher::utils::WatcherTest;
use anyhow::Result;
use pubky_common::crypto::Keypair;
use pubky_nexus::models::{
    post::PostStream,
    pubky_app::{traits::HashId, PubkyAppBookmark, PubkyAppPost, PubkyAppUser},
};

#[tokio::test]
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
        uri: format!("pubky://{}/pub/pubky.app/posts/{}", user_id, post_id),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let bookmark_blob = serde_json::to_vec(&bookmark)?;
    let bookmark_id = bookmark.create_id();
    let bookmark_url = format!(
        "pubky://{}/pub/pubky.app/bookmarks/{}",
        viewer_id, bookmark_id
    );

    // Put bookmark
    test.create_bookmark(&bookmark_url, bookmark_blob)
        .await
        .unwrap();

    // Step 4: Verify the bookmark exists in Nexus
    // GRAPH_OP: Assert if the event writes the graph
    let viewer_bookmark = find_post_bookmark(&user_id, &post_id, &viewer_id)
        .await
        .unwrap();
    assert_eq!(viewer_bookmark.id, bookmark_id);

    // INDEX_OP: Assert if the event writes the indexes
    let result_bookmarks = PostStream::get_bookmarked_posts(&viewer_id, None, None)
        .await
        .unwrap()
        .expect("The bookmark should have been created");

    assert_eq!(result_bookmarks.0.len(), 1);
    assert_eq!(result_bookmarks.0[0].details.id, post_id);

    // Cleanup user and post
    test.cleanup_post(&user_id, &post_id).await?;
    test.cleanup_user(&user_id).await?;
    test.cleanup_user(&viewer_id).await?;

    Ok(())
}
