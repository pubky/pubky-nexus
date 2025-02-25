use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky::Keypair;
use pubky_app_specs::{
    traits::HashId, PubkyAppBookmark, PubkyAppPost, PubkyAppPostKind, PubkyAppUser,
};
use pubky_nexus::{
    models::notification::{Notification, NotificationBody, PostChangedSource},
    types::Pagination,
    PubkyConnector,
};

#[tokio_shared_rt::test(shared)]
async fn test_edit_bookmarked_post_notification() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create User A who makes the original post
    let keypair_a = Keypair::random();
    let user_a = PubkyAppUser {
        bio: Some("User A bio".to_string()),
        image: None,
        links: None,
        name: "Watcher:BookmarkedPostEditNotification:UserA".to_string(),
        status: None,
    };
    let user_a_id = test.create_user(&keypair_a, &user_a).await?;

    // Create User B who bookmarks User A's post
    let keypair_b = Keypair::random();
    let user_b = PubkyAppUser {
        bio: Some("User B bio".to_string()),
        image: None,
        links: None,
        name: "Watcher:BookmarkedPostEditNotification:UserB".to_string(),
        status: None,
    };
    let user_b_id = test.create_user(&keypair_b, &user_b).await?;

    // User A creates a post
    let mut post = PubkyAppPost {
        content: "Original post by User A".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };
    let post_id = test.create_post(&user_a_id, &post).await?;

    // User B bookmarks User A's post
    let bookmark = PubkyAppBookmark {
        uri: format!("pubky://{}/pub/pubky.app/posts/{}", user_a_id, post_id),
        created_at: 0,
    };
    let bookmark_url = format!(
        "pubky://{}/pub/pubky.app/bookmarks/{}",
        user_b_id,
        bookmark.create_id()
    );
    test.put(&bookmark_url, bookmark).await?;

    // User A edits their post
    post.content = "Edited post by User A".to_string();
    let edited_url = format!("pubky://{}/pub/pubky.app/posts/{}", user_a_id, post_id);

    // Overwrite existing post in the homeserver for the edited one
    let pubky_client = PubkyConnector::get_pubky_client().await.unwrap();
    pubky_client
        .put(edited_url.as_str())
        .json(&post)
        .send()
        .await?;
    test.ensure_event_processing_complete().await?;

    // Verify that User B receives a notification about the edit
    let notifications = Notification::get_by_id(&user_b_id, Pagination::default())
        .await
        .unwrap();

    assert_eq!(
        notifications.len(),
        1,
        "User B should have exactly one notification"
    );

    if let NotificationBody::PostEdited {
        edit_source,
        edited_by,
        edited_uri,
        linked_uri,
    } = &notifications[0].body
    {
        assert_eq!(
            edited_by, &user_a_id,
            "Notification should specify the correct user who edited the post"
        );
        assert_eq!(
            edited_uri,
            &format!("pubky://{}/pub/pubky.app/posts/{}", user_a_id, post_id),
            "Notification should contain the correct edited post URI"
        );
        assert_eq!(
            linked_uri, &bookmark_url,
            "Notification should contain the correct bookmark URI"
        );
        assert_eq!(
            edit_source,
            &PostChangedSource::Bookmark,
            "Edit notification should have the correct source"
        );
    } else {
        panic!("Expected a PostEdited notification, found something else");
    }

    Ok(())
}
