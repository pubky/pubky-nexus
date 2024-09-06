use super::utils::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use pkarr::Keypair;
use pubky_nexus::models::{
    pubky_app::{PubkyAppFollow, PubkyAppUser},
    user::{Followers, Following, UserFollows},
};

#[tokio::test]
async fn test_homeserver_follow() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create first user (follower)
    let follower_keypair = Keypair::random();
    let follower_user = PubkyAppUser {
        bio: Some("This is the follower user".to_string()),
        image: None,
        links: None,
        name: "Follower User".to_string(),
        status: None,
    };
    let follower_id = test.create_user(&follower_keypair, &follower_user).await?;

    // Create second user (followee)
    let followee_keypair = Keypair::random();
    let followee_user = PubkyAppUser {
        bio: Some("This is the followee user".to_string()),
        image: None,
        links: None,
        name: "Followee User".to_string(),
        status: None,
    };
    let followee_id = test.create_user(&followee_keypair, &followee_user).await?;

    // Follow the followee
    let follow = PubkyAppFollow {
        created_at: Utc::now().timestamp_millis(),
    };
    let blob = serde_json::to_vec(&follow)?;
    let follow_url = format!(
        "pubky://{}/pub/pubky-app/follows/{}",
        follower_id, followee_id
    );
    test.client.put(follow_url.as_str(), &blob).await?;

    // Process the event
    test.ensure_event_processing_complete().await?;

    // Verify the new follower relationship exists in Nexus
    let result_followers = Followers::get_by_id(&followee_id, None, None)
        .await
        .unwrap()
        .expect("Followers should exist");
    assert_eq!(result_followers.0.len(), 1);
    assert_eq!(result_followers.0[0], follower_id);

    let result_following = Following::get_by_id(&follower_id, None, None)
        .await
        .unwrap()
        .expect("Following should exist");
    assert_eq!(result_following.0.len(), 1);
    assert_eq!(result_following.0[0], followee_id);

    // Unfollow the user
    test.client.delete(follow_url.as_str()).await?;

    // Process the event
    test.ensure_event_processing_complete().await?;

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
