use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use pkarr::Keypair;
use pubky_nexus::{
    models::notification::{Notification, NotificationBody},
    types::Pagination,
};

use pubky_app_specs::PubkyAppUser;

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_unfollow_notification() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Step 1: Create first user (follower)
    let follower_keypair = Keypair::random();

    let follower_user = PubkyAppUser {
        bio: Some("test_homeserver_unfollow_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:UnfollowNotification:Follower".to_string(),
        status: None,
    };
    let follower_id = test.create_user(&follower_keypair, &follower_user).await?;

    // Step 2: Create second user (followee)
    let followee_keypair = Keypair::random();

    let followee_user = PubkyAppUser {
        bio: Some("test_homeserver_unfollow_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:UnfollowNotification:Followee".to_string(),
        status: None,
    };
    let followee_id = test.create_user(&followee_keypair, &followee_user).await?;

    // Step 3: Follower follows the followee
    let follow_uri = test.create_follow(&follower_id, &followee_id).await?;
    // Step 4: Followee follows the follower back. To get notification of unfollow, users has to be friends
    let follow_back_uri = test.create_follow(&followee_id, &follower_id).await?;

    // Step 5: Follower unfollows the followee
    test.del(&follow_uri).await?;

    // Verify the followee gets a "Lost Friend" notification
    let notifications = Notification::get_by_id(&followee_id, Pagination::default())
        .await
        .unwrap();

    assert_eq!(
        notifications.len(),
        2,
        "Followee should have 2 notifications after unfollow"
    );
    if let NotificationBody::LostFriend { unfollowed_by } = &notifications[0].body {
        assert_eq!(
            unfollowed_by, &follower_id,
            "Notification should contain the correct follower"
        );
    } else {
        panic!("Expected a lost friend notification, found something else");
    }

    // Step 6: Followee unfollows the follower (no new notification should be generated)
    test.del(&follow_back_uri).await?;

    // Verify the follower gets no new notification after unfollow
    let notifications_follower = Notification::get_by_id(&follower_id, Pagination::default())
        .await
        .unwrap();
    assert_eq!(
        notifications_follower.len(),
        1,
        "Follower should have no new notifications after unfollow"
    );

    // Cleanup
    test.cleanup_user(&follower_id).await?;
    test.cleanup_user(&followee_id).await?;

    Ok(())
}
