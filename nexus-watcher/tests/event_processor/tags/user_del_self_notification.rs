use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::{models::notification::Notification, types::Pagination};
use pubky::Keypair;
use pubky_app_specs::{PubkyAppTag, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_self_untag_profile_no_notification() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create a single user who will tag and untag their own profile
    let user_kp = Keypair::random();

    let user = PubkyAppUser {
        bio: Some("test_homeserver_self_untag_profile_no_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:SelfUntagProfileNotification:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    // User tags their own profile
    let label = "self_tagged";

    let tag = PubkyAppTag {
        uri: format!("pubky://{user_id}/pub/pubky.app/profile.json"),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();

    // Put tag (self-tag should not produce a TagProfile notification)
    test.put(&user_kp, &tag_path, tag).await?;

    let notifications = Notification::get_by_id(&user_id, Pagination::default())
        .await
        .unwrap();
    assert_eq!(
        notifications.len(),
        0,
        "Self-tagging own profile should not produce a notification"
    );

    // Delete the tag (self-untag should not produce an UntagProfile notification)
    test.del(&user_kp, &tag_path).await?;

    let notifications = Notification::get_by_id(&user_id, Pagination::default())
        .await
        .unwrap();
    assert_eq!(
        notifications.len(),
        0,
        "Self-untagging own profile should not produce a notification"
    );

    // Cleanup
    test.cleanup_user(&user_kp).await?;

    Ok(())
}
