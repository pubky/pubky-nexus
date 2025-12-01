use super::utils::find_post_bookmark;
use crate::event_processor::utils::watcher::WatcherTest;
use crate::event_processor::{
    users::utils::find_user_counts, utils::watcher::HomeserverHashIdPath,
};
use anyhow::Result;
use nexus_common::models::event::Event;
use nexus_common::models::post::{Bookmark, PostStream};
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder, traits::HashId, PubkyAppBookmark, PubkyAppPost, PubkyAppUser,
};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_bookmark() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Step 1: Create a user
    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_homeserver_bookmark".to_string()),
        image: None,
        links: None,
        name: "Watcher:Bookmark:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;
    let (_, events_in_redis_before) = Event::get_events_from_redis(None, 1000).await.unwrap();

    // Step 2: Create a post under that user
    let post = PubkyAppPost {
        content: "Watcher:Bookmark:User:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let (post_id, post_path) = test.create_post(&user_kp, &post).await?;

    // Step 3: Add a bookmark to the post. Before create a new user
    let bookmark = PubkyAppBookmark {
        uri: post_uri_builder(user_id.clone(), post_id.clone()),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let bookmark_id = bookmark.create_id();
    let bookmark_path = bookmark.hs_path();

    // Put bookmark
    test.put(&user_kp, &bookmark_path, bookmark).await.unwrap();

    // Step 4: Verify the bookmark exists in Nexus
    // GRAPH_OP: Assert if the event writes the graph
    let user_bookmark = find_post_bookmark(&user_id, &post_id, &user_id)
        .await
        .unwrap();
    assert_eq!(user_bookmark.id, bookmark_id);

    // Verify bookmark counts have increased for this user
    let user_counts = find_user_counts(&user_id).await;
    assert_eq!(user_counts.bookmarks, 1);

    // INDEX_OP: Assert if the event writes the indexes
    let (_, events_in_redis_after) = Event::get_events_from_redis(None, 1000).await.unwrap();
    assert!(events_in_redis_after > events_in_redis_before);

    let result_bookmarks = PostStream::get_bookmarked_posts(
        &user_id,
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

    let exist_bookmark = Bookmark::get_from_index(&user_id, &post_id, &user_id)
        .await
        .unwrap();
    assert!(exist_bookmark.is_some(), "The bookmark has to be indexed");

    let bookmark = exist_bookmark.unwrap();
    assert_eq!(bookmark.id, bookmark_id, "Bookmark ids does not match");

    // Cleanup user and post
    test.cleanup_post(&user_kp, &post_path).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}
