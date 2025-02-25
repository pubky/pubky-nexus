use crate::watcher::follows::utils::find_follow_relationship;
use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky::Keypair;
use pubky_app_specs::PubkyAppUser;
use pubky_nexus::{
    models::user::{Relationship, UserCounts},
    RedisOps,
};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_unfollow_friend() -> Result<()> {
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

    // CACHE_OP: Assert if cache has been updated
    let follower_count = UserCounts::try_from_index_json(&[&follower_id], None)
        .await
        .unwrap()
        .expect("User count not found");

    assert_eq!(follower_count.followers, 1);
    assert_eq!(follower_count.following, 0);
    assert_eq!(follower_count.friends, 0);

    let following_count = UserCounts::try_from_index_json(&[&followee_id], None)
        .await
        .unwrap()
        .expect("User count not found");
    assert_eq!(following_count.followers, 0);
    assert_eq!(following_count.following, 1);
    assert_eq!(following_count.friends, 0);

    let relationship = Relationship::get_by_id(&follower_id, Some(&followee_id))
        .await
        .unwrap()
        .expect("User relationship not found");

    assert!(
        relationship.following,
        "Followee should be still following Follower"
    );
    assert!(
        !relationship.followed_by,
        "Follower should not be following Followee"
    );

    test.del(&follow_back_uri).await?;

    // GRAPH_OP: Check if relationship was deleted
    let exist = find_follow_relationship(&followee_id, &follower_id)
        .await
        .unwrap();
    assert!(!exist, "The follow edge not removed");

    // CACHE_OP: Assert if cache has been updated
    let follower_count = UserCounts::try_from_index_json(&[&follower_id], None)
        .await
        .unwrap()
        .expect("User count not found");

    assert_eq!(follower_count.followers, 0);
    assert_eq!(follower_count.following, 0);
    assert_eq!(follower_count.friends, 0);

    let following_count = UserCounts::try_from_index_json(&[&followee_id], None)
        .await
        .unwrap()
        .expect("User count not found");

    assert_eq!(following_count.followers, 0);
    assert_eq!(following_count.following, 0);
    assert_eq!(following_count.friends, 0);

    let relationship = Relationship::get_by_id(&follower_id, Some(&followee_id))
        .await
        .unwrap()
        .expect("User relationship not found");

    assert!(
        !relationship.following,
        "Followee should not be following Follower"
    );
    assert!(
        !relationship.followed_by,
        "Follower should not be following Followee"
    );

    // Cleanup
    test.cleanup_user(&follower_id).await?;
    test.cleanup_user(&followee_id).await?;

    Ok(())
}
