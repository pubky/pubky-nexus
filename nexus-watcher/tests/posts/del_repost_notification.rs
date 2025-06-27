use crate::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::{
    models::notification::{Notification, NotificationBody, PostChangedSource},
    types::Pagination,
};
use pubky::Keypair;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
async fn test_delete_post_that_reposted_notification() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create a user who posts
    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("Test user for post deletion".to_string()),
        image: None,
        links: None,
        name: "Watcher:RepostDeleteNotification:User".to_string(),
        status: None,
    };
    let poster_id = test.create_user(&keypair, &user).await?;

    // Create a user who posts
    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("Test user for post deletion".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostDeleteNotification:UserReposter".to_string(),
        status: None,
    };
    let reposter_id = test.create_user(&keypair, &user).await?;

    // Create a post without any relationships
    let post = PubkyAppPost {
        content: "User's post to be deleted".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };
    let post_id = test.create_post(&poster_id, &post).await?;

    // Create a repost
    let repost = PubkyAppPost {
        content: "User's post to be deleted".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: Some(PubkyAppPostEmbed {
            kind: PubkyAppPostKind::Short,
            uri: format!("pubky://{poster_id}/pub/pubky.app/posts/{post_id}"),
        }),
        attachments: None,
    };
    let repost_id = test.create_post(&reposter_id, &repost).await?;

    // Delete the repost
    test.cleanup_post(&reposter_id, &repost_id).await?;

    // Verify that the poster gets the correct notification
    let notifications = Notification::get_by_id(&poster_id, Pagination::default())
        .await
        .unwrap();

    assert_eq!(
        notifications.len(),
        2,
        "The poster should exactly have 2 notifications"
    );

    let notification = &notifications[0];
    if let NotificationBody::PostDeleted {
        delete_source,
        deleted_by,
        deleted_uri,
        linked_uri,
    } = &notification.body
    {
        assert_eq!(
            deleted_by, &reposter_id,
            "Notification should contain the correct deleter"
        );
        assert_eq!(
            deleted_uri,
            &format!("pubky://{reposter_id}/pub/pubky.app/posts/{repost_id}"),
            "Notification should contain the correct deleted post URI"
        );
        assert_eq!(
            linked_uri,
            &format!("pubky://{poster_id}/pub/pubky.app/posts/{post_id}"),
            "Notification should contain the correct linked post URI"
        );
        assert_eq!(
            delete_source,
            &PostChangedSource::Repost,
            "Delete notification should have the correct type"
        );
    } else {
        panic!("Expected a Deleted repost notification, found something else");
    }
    Ok(())
}
