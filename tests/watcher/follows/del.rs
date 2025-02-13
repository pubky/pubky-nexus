use super::utils::find_follow_relationship;
use crate::watcher::users::utils::find_user_counts;
use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use pkarr::Keypair;
use pubky_app_specs::PubkyAppUser;
use pubky_nexus::{
    models::{
        follow::{Followers, Following, UserFollows},
        user::Relationship,
    },
    RedisOps,
};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_unfollow() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create first user (follower)
    let follower_keypair = Keypair::random();

    let follower_user = PubkyAppUser {
        bio: Some("test_homeserver_unfollow".to_string()),
        image: None,
        links: None,
        name: "Watcher:Unfollow:Follower".to_string(),
        status: None,
    };
    let follower_id = test
        .create_user(&follower_keypair, &follower_user)
        .await
        .unwrap();

    // Create second user (followee)
    let followee_keypair = Keypair::random();
    let followee_user = PubkyAppUser {
        bio: Some("test_homeserver_unfollow".to_string()),
        image: None,
        links: None,
        name: "Watcher:Unfollow:Followee".to_string(),
        status: None,
    };
    let followee_id = test
        .create_user(&followee_keypair, &followee_user)
        .await
        .unwrap();

    // Follow the followee
    let follow_url = test.create_follow(&follower_id, &followee_id).await?;

    // Unfollow the followee
    test.del(&follow_url).await?;

    // GRAPH_OP: Check if relationship was deleted
    let exist = find_follow_relationship(&follower_id, &followee_id)
        .await
        .unwrap();

    assert!(!exist, "The follow edge not removed");

    // CACHE_OP: Assert the new follower relationship does not exist in the index
    let (_exist, member) = Followers::check_set_member(&[&followee_id], &follower_id)
        .await
        .unwrap();
    assert!(
        !member,
        "Followee user index cannot have Follower user in the followers list"
    );

    let (_exist, member) = Following::check_set_member(&[&follower_id], &followee_id)
        .await
        .unwrap();
    assert!(
        !member,
        "Follower user index cannot have Followee user in the following list"
    );

    // Check the relationship
    let relationship = Relationship::get_by_id(&followee_id, Some(&follower_id))
        .await
        .unwrap()
        .expect("Something went wrong");

    assert!(
        !relationship.following,
        "Follower cannot be following Followee"
    );
    assert!(
        !relationship.followed_by,
        "Followee cannot be following Follower"
    );

    let exist_count = find_user_counts(&followee_id).await;
    assert_eq!(exist_count.followers, 0);

    let exist_count = find_user_counts(&follower_id).await;
    assert_eq!(exist_count.following, 0);

    // GRAPH_OP: Verify the follower relationship no longer exists in Nexus
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
