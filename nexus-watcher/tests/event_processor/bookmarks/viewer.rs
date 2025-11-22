use super::utils::find_post_bookmark;
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use nexus_common::models::post::PostStream;
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder, traits::HashId, PubkyAppBookmark, PubkyAppPost, PubkyAppUser,
};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_viewer_bookmark() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

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
