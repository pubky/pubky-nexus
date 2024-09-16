use super::utils::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use pubky_common::crypto::Keypair;
use pubky_nexus::models::{
    notification::{Notification, NotificationBody},
    pubky_app::{PubkyAppFollow, PubkyAppUser},
};

#[tokio::test]
async fn test_homeserver_follow_notification() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Step 1: Create first user (follower)
    let follower_keypair = Keypair::random();
    let follower_user = PubkyAppUser {
        bio: Some("This is the follower user".to_string()),
        image: None,
        links: None,
        name: "Follower User".to_string(),
        status: None,
    };
    let follower_id = test.create_user(&follower_keypair, &follower_user).await?;

    // Step 2: Create second user (followee)
    let followee_keypair = Keypair::random();
    let followee_user = PubkyAppUser {
        bio: Some("This is the followee user".to_string()),
        image: None,
        links: None,
        name: "Followee User".to_string(),
        status: None,
    };
    let followee_id = test.create_user(&followee_keypair, &followee_user).await?;

    // Step 3: Follower follows the followee
    let follow = PubkyAppFollow {
        created_at: Utc::now().timestamp_millis(),
    };
    let blob = serde_json::to_vec(&follow)?;
    let follow_url = format!(
        "pubky://{}/pub/pubky.app/follows/{}",
        follower_id, followee_id
    );
    test.client.put(follow_url.as_str(), &blob).await?;
    test.ensure_event_processing_complete().await?;

    // Verify the followee gets a "New Follow" notification
    let notifications = Notification::get_by_id(&followee_id, None, None, None, None)
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
    let follow_back_url = format!(
        "pubky://{}/pub/pubky.app/follows/{}",
        followee_id, follower_id
    );
    test.client.put(follow_back_url.as_str(), &blob).await?;
    test.ensure_event_processing_complete().await?;

    // Verify the follower gets a "New Friend" notification
    let notifications_follower = Notification::get_by_id(&follower_id, None, None, None, None)
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

    // Step 5: Follower unfollows the followee
    test.client.delete(follow_url.as_str()).await?;
    test.ensure_event_processing_complete().await?;

    // Verify the followee gets a "Lost Friend" notification
    let notifications = Notification::get_by_id(&followee_id, None, None, None, None)
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
    test.client.delete(follow_back_url.as_str()).await?;
    test.ensure_event_processing_complete().await?;

    // Verify the follower gets no new notification after unfollow
    let notifications_follower = Notification::get_by_id(&follower_id, None, None, None, None)
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
