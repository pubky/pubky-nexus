use crate::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::{
    models::notification::{Notification, NotificationBody, PostChangedSource},
    types::Pagination,
};
use pubky::Keypair;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
async fn test_edit_reposted_post_notification() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create User A who makes the original post
    let keypair_a = Keypair::random();
    let user_a = PubkyAppUser {
        bio: Some("User A bio".to_string()),
        image: None,
        links: None,
        name: "Watcher:RepostedPostEditNotification:UserA".to_string(),
        status: None,
    };
    let user_a_id = test.create_user(&keypair_a, &user_a).await?;

    // Create User B who reposts User A's post
    let keypair_b = Keypair::random();
    let user_b = PubkyAppUser {
        bio: Some("User B bio".to_string()),
        image: None,
        links: None,
        name: "Watcher:RepostedPostEditNotification:UserB".to_string(),
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

    // User B reposts User A's post
    let repost = PubkyAppPost {
        content: "".to_string(), // Reposts usually have empty content
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: Some(PubkyAppPostEmbed {
            kind: PubkyAppPostKind::Short,
            uri: format!("pubky://{user_a_id}/pub/pubky.app/posts/{post_id}"),
        }),
        attachments: None,
    };
    let repost_id = test.create_post(&user_b_id, &repost).await?;

    // User A edits their post
    post.content = "Edited post by User A".to_string();
    let edited_url = format!("pubky://{user_a_id}/pub/pubky.app/posts/{post_id}");

    // Overwrite existing post in the homeserver for the edited one
    test.put(edited_url.as_str(), &post).await?;

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
            &format!("pubky://{user_a_id}/pub/pubky.app/posts/{post_id}"),
            "Notification should contain the correct edited post URI"
        );
        assert_eq!(
            linked_uri,
            &format!("pubky://{user_b_id}/pub/pubky.app/posts/{repost_id}"),
            "Notification should contain the correct repost URI"
        );
        assert_eq!(
            edit_source,
            &PostChangedSource::RepostEmbed,
            "Edit notification should have the correct source"
        );
    } else {
        panic!("Expected a PostEdited notification, found something else");
    }

    Ok(())
}
