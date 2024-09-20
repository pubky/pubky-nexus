use super::utils::find_follow_relationship;
use crate::watcher::{users::utils::find_user_counts, utils::WatcherTest};
use anyhow::Result;
use pubky_common::crypto::Keypair;
use pubky_nexus::{
    models::{
        pubky_app::PubkyAppUser,
        user::{Followers, Following, UserFollows},
    },
    RedisOps,
};

#[tokio::test]
async fn test_homeserver_follow() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create first user (follower)
    let follower_keypair = Keypair::random();

    let follower_user = PubkyAppUser {
        bio: Some("test_homeserver_follow".to_string()),
        image: None,
        links: None,
        name: "Watcher:Follow:Follower".to_string(),
        status: None,
    };
    let follower_id = test
        .create_user(&follower_keypair, &follower_user)
        .await
        .unwrap();

    // Create second user (followee)
    let followee_keypair = Keypair::random();
    let followee_user = PubkyAppUser {
        bio: Some("test_homeserver_follow".to_string()),
        image: None,
        links: None,
        name: "Watcher:Follow:Followee".to_string(),
        status: None,
    };
    let followee_id = test
        .create_user(&followee_keypair, &followee_user)
        .await
        .unwrap();

    // Follow the followee
    test.create_follow(&follower_id, &followee_id).await?;

    // GRAPH_OP: Check if relationship was created
    let exist = find_follow_relationship(&follower_id, &followee_id).await.unwrap();
    assert_eq!(
        exist, true,
        "The follow relationship was not created in the Graph"
    );

    // CACHE_OP: Assert the new follower relationship exists in Nexus
    let (_exist, member) = Followers::check_set_member(&[&followee_id], &follower_id)
        .await
        .unwrap();
    assert!(member);

    let (_exist, member) = Following::check_set_member(&[&follower_id], &followee_id)
        .await
        .unwrap();
    assert!(member);

    // CACHE_OP: Assert if cache has been updated
    let exist_count = find_user_counts(&followee_id).await;
    assert_eq!(exist_count.followers, 1);

    let exist_count = find_user_counts(&follower_id).await;
    assert_eq!(exist_count.following, 1);

    // Unfollow the user
    test.delete_follow(&follower_id, &followee_id).await?;

    // Verify the follower relationship no longer exists in Nexus
    let result_followers = Followers::get_by_id(&followee_id, None, None)
        .await
        .unwrap();

    assert!(
        result_followers.is_none() || result_followers.unwrap().0.is_empty(),
        "The followee should have no followers"
    );

    let result_following = Following::get_by_id(&follower_id, None, None)
        .await
        .unwrap();
    assert!(
        result_following.is_none() || result_following.unwrap().0.is_empty(),
        "The follower should not be following anyone"
    );

    // Cleanup
    test.cleanup_user(&follower_id).await?;
    test.cleanup_user(&followee_id).await?;

    Ok(())
}
