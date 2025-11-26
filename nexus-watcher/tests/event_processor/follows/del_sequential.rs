use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::db::RedisOps;
use nexus_common::models::notification::{Notification, NotificationBody};
use nexus_common::models::user::Relationship;
use nexus_common::models::user::UserCounts;
use nexus_common::types::Pagination;
use pubky::Keypair;
use pubky_app_specs::PubkyAppUser;

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_sequential_unfollow() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create followee
    let followee_kp = Keypair::random();

    let followee_user = PubkyAppUser {
        bio: Some("test_homeserver_sequential_follow".to_string()),
        image: None,
        links: None,
        name: "Watcher:FollowChain:Followee".to_string(),
        status: None,
    };

    let followee_id = test.create_user(&followee_kp, &followee_user).await?;

    // Create Alice user
    let alice_kp = Keypair::random();
    let alice_user = PubkyAppUser {
        bio: Some("test_homeserver_sequential_follow".to_string()),
        image: None,
        links: None,
        name: "Watcher:FollowChain:Alice".to_string(),
        status: None,
    };
    let _alice_id = test.create_user(&alice_kp, &alice_user).await?;

    // Follow followee
    let alice_follow_uri = test.create_follow(&alice_kp, &followee_id).await?;

    // Create Bob user
    let bob_kp = Keypair::random();
    let bob_user = PubkyAppUser {
        bio: Some("test_homeserver_sequential_follow".to_string()),
        image: None,
        links: None,
        name: "Watcher:FollowChain:Bob".to_string(),
        status: None,
    };
    let _bob_id = test.create_user(&bob_kp, &bob_user).await?;

    // Follow followee
    let bob_follow_uri = test.create_follow(&bob_kp, &followee_id).await?;

    let carla_kp = Keypair::random();
    let carla_user = PubkyAppUser {
        bio: Some("test_homeserver_sequential_follow".to_string()),
        image: None,
        links: None,
        name: "Watcher:FollowChain:Carla".to_string(),
        status: None,
    };
    let _carla_id = test.create_user(&carla_kp, &carla_user).await?;

    // Follow followee
    let carla_follow_uri = test.create_follow(&carla_kp, &followee_id).await?;

    let danonino_kp = Keypair::random();
    let danonino_user = PubkyAppUser {
        bio: Some("test_homeserver_sequential_follow".to_string()),
        image: None,
        links: None,
        name: "Watcher:FollowChain:Danonino".to_string(),
        status: None,
    };
    let danonino_id = test.create_user(&danonino_kp, &danonino_user).await?;

    // Follow followee
    let danonino_follow_uri = test.create_follow(&danonino_kp, &followee_id).await?;

    // Create Enzo user
    let enzo_kp = Keypair::random();
    let enzo_user = PubkyAppUser {
        bio: Some("test_homeserver_sequential_follow".to_string()),
        image: None,
        links: None,
        name: "Watcher:FollowChain:Enzo".to_string(),
        status: None,
    };
    let enzo_id = test.create_user(&enzo_kp, &enzo_user).await?;

    // Followee Follow Danonino
    let followee_follow_danonino_uri = test.create_follow(&followee_kp, &danonino_id).await?;

    // Follow Enzo
    let followee_follow_enzo_uri = test.create_follow(&followee_kp, &enzo_id).await?;

    // Start unfollowing users
    test.del(&alice_kp, &alice_follow_uri).await?;
    test.del(&bob_kp, &bob_follow_uri).await?;

    // Assert folowee counts
    let follower_count = UserCounts::try_from_index_json(&[&followee_id], None)
        .await
        .unwrap()
        .expect("User count not found");

    assert_eq!(follower_count.followers, 2);
    assert_eq!(follower_count.following, 2);
    assert_eq!(follower_count.friends, 1);

    test.del(&followee_kp, &followee_follow_danonino_uri)
        .await?;

    // Assert folowee relationships and notifications
    let relationship = Relationship::get_by_id(&danonino_id, Some(&followee_id))
        .await
        .unwrap()
        .expect("User relationship not found");
    assert!(
        !relationship.following,
        "Followee cannot be following Danonino"
    );
    assert!(
        relationship.followed_by,
        "Danonino should be following Followee"
    );

    let notifications_danonino = Notification::get_by_id(&danonino_id, Pagination::default())
        .await
        .unwrap();

    assert_eq!(
        notifications_danonino.len(),
        2,
        "Follower should have 2 notifications: NewFriend and LostFriend"
    );

    if let NotificationBody::NewFriend { followed_by } = &notifications_danonino[1].body {
        assert_eq!(
            followed_by, &followee_id,
            "Notification should contain the correct followee"
        );
    } else {
        panic!("Expected a new friend notification, found something else");
    }

    if let NotificationBody::LostFriend { unfollowed_by } = &notifications_danonino[0].body {
        assert_eq!(
            unfollowed_by, &followee_id,
            "Notification should contain the correct follower"
        );
    } else {
        panic!("Expected a new friend notification, found something else");
    }

    test.del(&carla_kp, &carla_follow_uri).await?;
    test.del(&danonino_kp, &danonino_follow_uri).await?;
    test.del(&followee_kp, &followee_follow_enzo_uri).await?;

    // Assert folowee last counts
    let follower_count = UserCounts::try_from_index_json(&[&followee_id], None)
        .await
        .unwrap()
        .expect("User count not found");

    assert_eq!(follower_count.followers, 0);
    assert_eq!(follower_count.following, 0);
    assert_eq!(follower_count.friends, 0);

    let relationship = Relationship::get_by_id(&followee_id, Some(&enzo_id))
        .await
        .unwrap()
        .expect("User relationship not found");
    assert!(
        !relationship.following,
        "Enzo should not be following followee"
    );
    assert!(
        !relationship.followed_by,
        "Followee cannot be following Enzo"
    );

    // Cleanup
    test.cleanup_user(&followee_kp).await?;
    test.cleanup_user(&alice_kp).await?;
    test.cleanup_user(&bob_kp).await?;
    test.cleanup_user(&carla_kp).await?;
    test.cleanup_user(&danonino_kp).await?;
    test.cleanup_user(&enzo_kp).await?;
    // TODO: Clear Follows

    Ok(())
}
