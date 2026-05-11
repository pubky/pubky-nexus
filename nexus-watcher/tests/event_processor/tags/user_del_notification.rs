use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::{
    models::notification::{Notification, NotificationBody},
    types::Pagination,
};
use pubky::Keypair;
use pubky_app_specs::{PubkyAppTag, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_untag_user_notification() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create the first user (tagged user)
    let tagged_kp = Keypair::random();

    let tagged_user = PubkyAppUser {
        bio: Some("test_homeserver_untag_user_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:UntagUserNotification:TaggedUser".to_string(),
        status: None,
    };
    let tagged_user_id = test.create_user(&tagged_kp, &tagged_user).await?;

    // Create the second user (tagger)
    let tagger_kp = Keypair::random();

    let tagger_user = PubkyAppUser {
        bio: Some("test_homeserver_untag_user_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:UntagUserNotification:TaggerUser".to_string(),
        status: None,
    };
    let tagger_user_id = test.create_user(&tagger_kp, &tagger_user).await?;

    // Tagger adds a tag to the profile of the tagged user
    let label = "cheerful";

    let tag = PubkyAppTag {
        uri: format!("pubky://{tagged_user_id}/pub/pubky.app/profile.json"),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();

    // Put tag
    test.put(&tagger_kp, &tag_path, tag).await?;

    // Verify the TagProfile notification was created
    let notifications = Notification::get_by_id(&tagged_user_id, Pagination::default())
        .await
        .unwrap();
    assert_eq!(
        notifications.len(),
        1,
        "Tagged user should have 1 notification for the new tag"
    );

    // Delete the tag
    test.del(&tagger_kp, &tag_path).await?;

    // Check if the tagged user now has a new UntagProfile notification
    let notifications = Notification::get_by_id(&tagged_user_id, Pagination::default())
        .await
        .unwrap();
    assert_eq!(
        notifications.len(),
        2,
        "Tagged user should have 2 notifications: tag + untag"
    );

    // The most recent notification (first in descending order) should be UntagProfile
    let notification = &notifications[0];
    if let NotificationBody::UntagProfile {
        untagged_by,
        tag_label,
    } = &notification.body
    {
        assert_eq!(
            untagged_by, &tagger_user_id,
            "Notification should contain the correct untagger"
        );
        assert_eq!(
            tag_label, label,
            "Notification should contain the correct tag label"
        );
    } else {
        panic!("Expected an untag profile notification, found something else");
    }

    // Cleanup
    test.cleanup_user(&tagged_kp).await?;
    test.cleanup_user(&tagger_kp).await?;

    Ok(())
}
