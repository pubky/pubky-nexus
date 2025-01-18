use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky_app_specs::PubkyAppUser;
use pubky_common::crypto::Keypair;
use pubky_nexus::models::user::Relationship;
use pubky_nexus::models::user::UserCounts;
use pubky_nexus::RedisOps;

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_sequential_follow() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create followee
    let followee_keypair = Keypair::random();

    let followee_user = PubkyAppUser {
        bio: Some("test_homeserver_sequential_follow".to_string()),
        image: None,
        links: None,
        name: "Watcher:FollowChain:Followee".to_string(),
        status: None,
    };

    let followee_id = test.create_user(&followee_keypair, &followee_user).await?;

    // Create Bob user
    let bob_keypair = Keypair::random();
    let bob_user = PubkyAppUser {
        bio: Some("test_homeserver_sequential_follow".to_string()),
        image: None,
        links: None,
        name: "Watcher:FollowChain:Bob".to_string(),
        status: None,
    };
    let bob_id = test.create_user(&bob_keypair, &bob_user).await?;

    // Follow followee
    test.create_follow(&bob_id, &followee_id).await?;

    let relationship = Relationship::get_by_id(&followee_id, Some(&bob_id))
        .await
        .unwrap()
        .expect("User relationship not found");
    assert!(relationship.following, "Bob should be following followee");
    assert!(
        !relationship.followed_by,
        "Followee cannot be following Bob"
    );

    // Create Alice user
    let alice_keypair = Keypair::random();
    let alice_user = PubkyAppUser {
        bio: Some("test_homeserver_sequential_follow".to_string()),
        image: None,
        links: None,
        name: "Watcher:FollowChain:Alice".to_string(),
        status: None,
    };
    let alice_id = test.create_user(&alice_keypair, &alice_user).await?;

    // Follow followee
    test.create_follow(&alice_id, &followee_id).await?;

    let relationship = Relationship::get_by_id(&followee_id, Some(&alice_id))
        .await
        .unwrap()
        .expect("User relationship not found");
    assert!(relationship.following, "Alice should be following followee");
    assert!(
        !relationship.followed_by,
        "Followee cannot be following Alice"
    );

    // Create Carla user
    let carla_keypair = Keypair::random();
    let carla_user = PubkyAppUser {
        bio: Some("test_homeserver_sequential_follow".to_string()),
        image: None,
        links: None,
        name: "Watcher:FollowChain:Carla".to_string(),
        status: None,
    };
    let carla_id = test.create_user(&carla_keypair, &carla_user).await?;

    // Follow followee
    test.create_follow(&carla_id, &followee_id).await?;

    let relationship = Relationship::get_by_id(&followee_id, Some(&carla_id))
        .await
        .unwrap()
        .expect("User relationship not found");
    assert!(relationship.following, "Carla should be following followee");
    assert!(
        !relationship.followed_by,
        "Followee cannot be following Carla"
    );

    // Create Danonino user
    let danonino_keypair = Keypair::random();
    let danonino_user = PubkyAppUser {
        bio: Some("test_homeserver_sequential_follow".to_string()),
        image: None,
        links: None,
        name: "Watcher:FollowChain:Danonino".to_string(),
        status: None,
    };
    let danonino_id = test.create_user(&danonino_keypair, &danonino_user).await?;

    // Follow followee
    test.create_follow(&danonino_id, &followee_id).await?;

    let relationship = Relationship::get_by_id(&followee_id, Some(&danonino_id))
        .await
        .unwrap()
        .expect("User relationship not found");
    assert!(
        relationship.following,
        "Danonino should be following followee"
    );
    assert!(
        !relationship.followed_by,
        "Followee cannot be following Danonino"
    );

    let followee_user_count = UserCounts::try_from_index_json(None, &[&followee_id])
        .await
        .unwrap()
        .expect("User count not found");

    assert_eq!(followee_user_count.followers, 4);
    assert_eq!(followee_user_count.following, 0);
    assert_eq!(followee_user_count.friends, 0);

    // Follow Danonino
    test.create_follow(&followee_id, &danonino_id).await?;

    let relationship = Relationship::get_by_id(&danonino_id, Some(&followee_id))
        .await
        .unwrap()
        .expect("User relationship not found");
    assert!(
        relationship.following,
        "Followee should be following Danonino"
    );
    assert!(
        relationship.followed_by,
        "Danonino should be following Followee"
    );

    let followee_user_count = UserCounts::try_from_index_json(None, &[&followee_id])
        .await
        .unwrap()
        .expect("User count not found");

    assert_eq!(followee_user_count.followers, 4);
    assert_eq!(followee_user_count.following, 1);
    assert_eq!(followee_user_count.friends, 1);

    // Create Enzo user
    let enzo_keypair = Keypair::random();
    let enzo_user = PubkyAppUser {
        bio: Some("test_homeserver_sequential_follow".to_string()),
        image: None,
        links: None,
        name: "Watcher:FollowChain:Enzo".to_string(),
        status: None,
    };
    let enzo_id = test.create_user(&enzo_keypair, &enzo_user).await?;

    // Follow Enzo
    test.create_follow(&followee_id, &enzo_id).await?;

    let relationship = Relationship::get_by_id(&enzo_id, Some(&followee_id))
        .await
        .unwrap()
        .expect("User relationship not found");
    assert!(relationship.following, "Followee should be following Enzo");
    assert!(
        !relationship.followed_by,
        "Enzo should not be following Followee"
    );

    let relationship = Relationship::get_by_id(&followee_id, Some(&enzo_id))
        .await
        .unwrap()
        .expect("User relationship not found");
    assert!(
        !relationship.following,
        "Enzo should not be following Followee"
    );
    assert!(
        relationship.followed_by,
        "Followee should be following Enzo"
    );

    let followee_user_count = UserCounts::try_from_index_json(None, &[&followee_id])
        .await
        .unwrap()
        .expect("User count not found");

    assert_eq!(followee_user_count.followers, 4);
    assert_eq!(followee_user_count.following, 2);
    assert_eq!(followee_user_count.friends, 1);

    // Cleanup
    test.cleanup_user(&followee_id).await?;
    test.cleanup_user(&bob_id).await?;
    test.cleanup_user(&alice_id).await?;
    test.cleanup_user(&carla_id).await?;
    test.cleanup_user(&danonino_id).await?;
    test.cleanup_user(&enzo_id).await?;
    // TODO: Clear Follows

    Ok(())
}
