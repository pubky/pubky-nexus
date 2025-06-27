use crate::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use nexus_common::{
    models::notification::{Notification, NotificationBody},
    types::Pagination,
};
use pubky::Keypair;
use pubky_app_specs::{traits::HashId, PubkyAppTag, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_put_tag_user_notification() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create the first user (tagged user)
    let tagged_keypair = Keypair::random();

    let tagged_user = PubkyAppUser {
        bio: Some("test_homeserver_put_tag_user_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:PutTagNotification:TaggedUser".to_string(),
        status: None,
    };
    let tagged_user_id = test.create_user(&tagged_keypair, &tagged_user).await?;

    // Create the second user (tagger)
    let tagger_keypair = Keypair::random();

    let tagger_user = PubkyAppUser {
        bio: Some("test_homeserver_put_tag_user_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:PutTagNotification:TaggerUser".to_string(),
        status: None,
    };
    let tagger_user_id = test.create_user(&tagger_keypair, &tagger_user).await?;

    // Tagger adds a tag to the profile of the tagged user
    let label = "friendly";

    let tag = PubkyAppTag {
        uri: format!("pubky://{tagged_user_id}/pub/pubky.app/profile.json"),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_url = format!(
        "pubky://{}/pub/pubky.app/tags/{}",
        tagger_user_id,
        tag.create_id()
    );

    // Put tag
    test.put(tag_url.as_str(), tag).await?;

    // Check if the tagged user received a notification
    let notifications = Notification::get_by_id(&tagged_user_id, Pagination::default())
        .await
        .unwrap();
    assert_eq!(
        notifications.len(),
        1,
        "Tagged user should have 1 notification"
    );

    let notification = &notifications[0];
    if let NotificationBody::TagProfile {
        tagged_by,
        tag_label,
    } = &notification.body
    {
        assert_eq!(
            tagged_by, &tagger_user_id,
            "Notification should contain the correct tagger"
        );
        assert_eq!(
            tag_label, label,
            "Notification should contain the correct tag label"
        );
    } else {
        panic!("Expected a tag profile notification, found something else");
    }

    // Cleanup
    test.cleanup_user(&tagged_user_id).await?;
    test.cleanup_user(&tagger_user_id).await?;

    Ok(())
}
