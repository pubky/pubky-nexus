use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky_app_specs::PubkyAppUser;
use pubky_common::crypto::Keypair;
use pubky_nexus::models::user::Relationship;
use pubky_nexus::models::user::UserCounts;
use pubky_nexus::RedisOps;

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_follow_friend() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create Alice user
    let alice_keypair = Keypair::random();

    let alice_user = PubkyAppUser {
        bio: Some("test_homeserver_follow_friend".to_string()),
        image: None,
        links: None,
        name: "Watcher:FollowFriend:Alice".to_string(),
        status: None,
    };

    let alice_id = test.create_user(&alice_keypair, &alice_user).await?;

    // Create Bob user
    let bob_keypair = Keypair::random();
    let bob_user = PubkyAppUser {
        bio: Some("test_homeserver_follow_friend".to_string()),
        image: None,
        links: None,
        name: "Watcher:FollowFriend:Bob".to_string(),
        status: None,
    };
    let bob_id = test.create_user(&bob_keypair, &bob_user).await?;

    // Follow Alice
    test.create_follow(&bob_id, &alice_id).await?;

    let relationship = Relationship::get_by_id(&alice_id, Some(&bob_id))
        .await
        .unwrap()
        .expect("User relationship not found");
    assert!(relationship.following, "Bob should be following Alice");
    assert!(!relationship.followed_by, "Alice cannot be following Bob");

    // CACHE_OP: Assert if cache has been updated
    let alice_user_count = UserCounts::try_from_index_json(&[&alice_id], None)
        .await
        .unwrap()
        .expect("User count not found");

    assert_eq!(alice_user_count.followers, 1);
    assert_eq!(alice_user_count.following, 0);
    assert_eq!(alice_user_count.friends, 0);

    let bob_user_count = UserCounts::try_from_index_json(&[&bob_id], None)
        .await
        .unwrap()
        .expect("User count not found");
    assert_eq!(bob_user_count.following, 1);
    assert_eq!(bob_user_count.followers, 0);
    assert_eq!(bob_user_count.friends, 0);

    // Follow Bob
    test.create_follow(&alice_id, &bob_id).await?;

    let relationship = Relationship::get_by_id(&bob_id, Some(&alice_id))
        .await
        .unwrap()
        .expect("User relationship not found");

    assert!(
        relationship.following,
        "Bob should be already following Alice"
    );
    assert!(relationship.followed_by, "Alice should be following Bob");

    // Now Alice and Bob are friends
    // CACHE_OP: Assert if cache has been updated
    let alice_user_count = UserCounts::try_from_index_json(&[&bob_id], None)
        .await
        .unwrap()
        .expect("User count not found");
    assert_eq!(alice_user_count.followers, 1);
    assert_eq!(alice_user_count.following, 1);
    assert_eq!(alice_user_count.friends, 1);

    let bob_user_count = UserCounts::try_from_index_json(&[&bob_id], None)
        .await
        .unwrap()
        .expect("User count not found");
    assert_eq!(bob_user_count.following, 1);
    assert_eq!(bob_user_count.following, 1);
    assert_eq!(bob_user_count.friends, 1);

    // Cleanup
    test.cleanup_user(&alice_id).await?;
    test.cleanup_user(&bob_id).await?;
    // TODO: Clear Follows

    Ok(())
}
