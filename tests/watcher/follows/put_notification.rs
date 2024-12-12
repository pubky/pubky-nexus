use crate::watcher::utils::WatcherTest;
use anyhow::Result;
use pubky_app_specs::PubkyAppUser;
use pubky_common::crypto::Keypair;
use pubky_nexus::{
    models::notification::{Notification, NotificationBody},
    types::Pagination,
};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_follow_notification() -> Result<()> {
    let mut test = WatcherTest::setup(false).await?;

    // Step 1: Create first user (follower)
    let follower_keypair = Keypair::random();

    let follower_user = PubkyAppUser {
        bio: Some("test_homeserver_follow_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:FollowNotification:Follower".to_string(),
        status: None,
    };
    let follower_id = test.create_user(&follower_keypair, &follower_user).await?;

    // Step 2: Create second user (followee)
    let followee_keypair = Keypair::random();

    let followee_user = PubkyAppUser {
        bio: Some("test_homeserver_follow_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:FollowNotification:Followee".to_string(),
        status: None,
    };
    let followee_id = test.create_user(&followee_keypair, &followee_user).await?;

    // Step 3: Follower follows the followee
    test.create_follow(&follower_id, &followee_id).await?;

    // Verify the followee gets a "New Follow" notification
    let notifications = Notification::get_by_id(&followee_id, Pagination::default())
        .await
        .unwrap();

    assert_eq!(
        notifications.len(),
        1,
        "Followee should have 1 notification"
    );

    let notification = &notifications[0];

    if let NotificationBody::Follow { followed_by } = &notification.body {
        assert_eq!(
            followed_by, &follower_id,
            "Notification should contain the correct follower"
        );
    } else {
        panic!("Expected a follow notification, found something else");
    }

    // Step 4: Followee follows the follower back
    test.create_follow(&followee_id, &follower_id).await?;

    // Verify the follower gets a "New Friend" notification
    let notifications_follower = Notification::get_by_id(&follower_id, Pagination::default())
        .await
        .unwrap();

    assert_eq!(
        notifications_follower.len(),
        1,
        "Follower should have 1 new friend notification"
    );

    if let NotificationBody::NewFriend { followed_by } = &notifications_follower[0].body {
        assert_eq!(
            followed_by, &followee_id,
            "Notification should contain the correct followee"
        );
    } else {
        panic!("Expected a new friend notification, found something else");
    }

    // Cleanup
    test.cleanup_user(&follower_id).await?;
    test.cleanup_user(&followee_id).await?;

    Ok(())
}
