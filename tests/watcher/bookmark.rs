use super::utils::WatcherTest;
use anyhow::Result;
use pkarr::Keypair;
use pubky_nexus::models::{
    post::PostStream,
    pubky_app::{traits::GenerateId, PubkyAppBookmark, PubkyAppPost, PubkyAppUser},
};

#[tokio::test]
async fn test_homeserver_bookmark() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Step 1: Create a user
    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("This is a bookmark test user".to_string()),
        image: None,
        links: None,
        name: "Test User Bookmark".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    // Step 2: Create a post under that user
    let post = PubkyAppPost {
        content: "This is a bookmark test post!".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
    };
    let post_id = test.create_post(&user_id, &post).await?;

    // Verify the post exists and is indexed correctly
    // let result_post = PostView::get_by_id(&user_id, &post_id, None, None, None)
    //     .await
    //     .unwrap()
    //     .expect("The new post was not served from Nexus");
    // assert_eq!(result_post.details.id, post_id);
    // assert_eq!(result_post.details.content, post.content);

    // Step 3: Add a bookmark to the post
    let bookmark = PubkyAppBookmark {
        uri: format!("pubky://{}/pub/pubky.app/posts/{}", user_id, post_id),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let bookmark_blob = serde_json::to_vec(&bookmark)?;
    let bookmark_url = format!(
        "pubky://{}/pub/pubky.app/bookmarks/{}",
        user_id,
        bookmark.create_id()
    );

    // Put bookmark
    test.client
        .put(bookmark_url.as_str(), &bookmark_blob)
        .await?;
    test.ensure_event_processing_complete().await?;

    // Step 4: Verify the bookmark exists in Nexus
    let result_bookmarks = PostStream::get_bookmarked_posts(&user_id, None, None)
        .await
        .unwrap()
        .expect("The bookmark should have been created");

    assert_eq!(result_bookmarks.0.len(), 1);
    assert_eq!(result_bookmarks.0[0].details.id, post_id);

    // Step 5: Delete the bookmark
    test.client.delete(bookmark_url.as_str()).await?;
    test.ensure_event_processing_complete().await?;

    // Step 6: Verify the bookmark has been deleted
    let _result_bookmarks_after_delete = PostStream::get_bookmarked_posts(&user_id, None, None)
        .await
        .unwrap();
    // TODO: handle delete bookmark from Redis
    // assert!(
    //     result_bookmarks_after_delete.is_none(),
    //     "The bookmark should have been deleted"
    // );

    // Cleanup user and post
    test.cleanup_post(&user_id, &post_id).await?;
    test.cleanup_user(&user_id).await?;

    Ok(())
}
