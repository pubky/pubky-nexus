use crate::event_processor::utils::watcher::{
    assert_eventually_exists, HomeserverHashIdPath, WatcherTest,
};
use anyhow::Result;
use nexus_watcher::events::retry::{RetryEvent, RetryEventIndexKey};
use pubky::Keypair;
use pubky_app_specs::{
    bookmark_uri_builder, post_uri_builder, traits::HashId, PubkyAppBookmark, PubkyAppUser,
};

/// The user profile is stored in the homeserver. Missing the post to connect the bookmark
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_bookmark_cannot_index() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_homeserver_bookmark_cannot_index".to_string()),
        image: None,
        links: None,
        name: "Watcher:IndexFail:Bookmark:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    // Use a placeholder parent post ID to intentionally avoid resolving it in the graph database
    let fake_post_id = "0032QB10HCRHG";
    let fake_user_id = "ba3e8qeby33uq9cughpxdf7bew9etn1eq8bc3yhwg7p1f54yaozy";
    // Create parent post uri
    let post_absolute_uri = post_uri_builder(fake_user_id.into(), fake_post_id.into());

    // Create a bookmark content
    let bookmark = PubkyAppBookmark {
        uri: post_absolute_uri,
        created_at: chrono::Utc::now().timestamp_millis(),
    };

    // Create the bookmark of the shadow user
    let bookmark_path = bookmark.hs_path();
    let bookmark_id = bookmark.create_id();
    let bookmark_absolute_url = bookmark_uri_builder(user_id, bookmark_id);
    // PUT bookmark
    test.put(&user_kp, &bookmark_path, bookmark).await?;

    let index_key: RetryEventIndexKey = bookmark_absolute_url.clone();

    assert_eventually_exists(&index_key).await;

    assert!(RetryEvent::check_uri(&index_key).await.unwrap());

    let event_retry = RetryEvent::get_from_index(&index_key).await.unwrap();
    assert!(event_retry.is_some());

    let event_state = event_retry.unwrap();
    assert_eq!(event_state.retry_count, 0);
    assert_eq!(event_state.event_uri, bookmark_absolute_url);

    // DEL bookmark — bookmark was never indexed, so DEL returns Ok (no-op, no retry created)
    test.del(&user_kp, &bookmark_path).await?;

    Ok(())
}
