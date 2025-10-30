use super::utils::find_follow_relationship;
use crate::event_processor::users::utils::find_user_counts;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::{
    db::RedisOps,
    models::{
        follow::{Followers, Following},
        user::Relationship,
    },
};
use pubky::Keypair;
use pubky_app_specs::PubkyAppUser;

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_put_follow() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create first user (follower)
    let follower_kp = Keypair::random();

    let follower_user = PubkyAppUser {
        bio: Some("test_homeserver_raw_follow".to_string()),
        image: None,
        links: None,
        name: "Watcher:Follow:Follower".to_string(),
        status: None,
    };
    let follower_id = test
        .create_user(&follower_kp, &follower_user)
        .await
        .unwrap();

    // Create second user (followee)
    let followee_kp = Keypair::random();
    let followee_user = PubkyAppUser {
        bio: Some("test_homeserver_raw_follow".to_string()),
        image: None,
        links: None,
        name: "Watcher:Follow:Followee".to_string(),
        status: None,
    };
    let followee_id = test
        .create_user(&followee_kp, &followee_user)
        .await
        .unwrap();

    // Follow the followee
    test.create_follow(&follower_kp, &followee_id).await?;

    // GRAPH_OP: Check if relationship was created
    let exist = find_follow_relationship(&follower_id, &followee_id)
        .await
        .unwrap();
    assert!(
        exist,
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

    let relationship = Relationship::get_by_id(&followee_id, Some(&follower_id))
        .await
        .unwrap()
        .expect("User relationship not found");
    assert!(
        relationship.following,
        "Follower should be following Followee"
    );
    assert!(
        !relationship.followed_by,
        "Followee cannot be following Follower"
    );

    // CACHE_OP: Assert if cache has been updated
    let exist_count = find_user_counts(&followee_id).await;
    assert_eq!(exist_count.followers, 1);

    let exist_count = find_user_counts(&follower_id).await;
    assert_eq!(exist_count.following, 1);

    // Cleanup
    test.cleanup_user(&follower_kp).await?;
    test.cleanup_user(&followee_kp).await?;

    Ok(())
}
