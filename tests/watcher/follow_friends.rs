use super::utils::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use pkarr::Keypair;
use pubky_nexus::{
    models::{
        pubky_app::{PubkyAppFollow, PubkyAppUser},
        user::UserCounts,
    },
    RedisOps,
};

#[tokio::test]
async fn test_homeserver_follow_friend() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create Alice user
    let alice_keypair = Keypair::random();

    let alice_user = PubkyAppUser {
        bio: Some("I am Alice".to_string()),
        image: None,
        links: None,
        name: "Alcie".to_string(),
        status: None,
    };

    let alice_id = test.create_user(&alice_keypair, &alice_user).await?;

    // Create Bob user
    let bob_keypair = Keypair::random();
    let bob_user = PubkyAppUser {
        bio: Some("I am Bob".to_string()),
        image: None,
        links: None,
        name: "Bob".to_string(),
        status: None,
    };
    let bob_id = test.create_user(&bob_keypair, &bob_user).await?;

    // Follow Bob
    let follow_bob = PubkyAppFollow {
        created_at: Utc::now().timestamp_millis(),
    };
    let blob = serde_json::to_vec(&follow_bob)?;
    let bob_follow_url = format!("pubky://{}/pub/pubky.app/follows/{}", alice_id, bob_id);
    test.client.put(bob_follow_url.as_str(), &blob).await?;
    // Process the event
    test.ensure_event_processing_complete().await?;

    let bob_user_count = UserCounts::try_from_index_json(&[&bob_id])
        .await
        .unwrap()
        .expect("User count not found");
    assert_eq!(bob_user_count.followers, 1);
    assert_eq!(bob_user_count.following, 0);
    assert_eq!(bob_user_count.friends, 0);

    let alice_user_count = UserCounts::try_from_index_json(&[&alice_id])
        .await
        .unwrap()
        .expect("User count not found");
    assert_eq!(alice_user_count.following, 1);
    assert_eq!(alice_user_count.followers, 0);
    assert_eq!(alice_user_count.friends, 0);

    // Follow Alice
    let follow_alice = PubkyAppFollow {
        created_at: Utc::now().timestamp_millis(),
    };
    let blob = serde_json::to_vec(&follow_alice)?;
    let alice_follow_url = format!("pubky://{}/pub/pubky.app/follows/{}", bob_id, alice_id);
    test.client.put(alice_follow_url.as_str(), &blob).await?;
    // Process the event
    test.ensure_event_processing_complete().await?;

    // Now Alice and Bob are friends
    let alice_user_count = UserCounts::try_from_index_json(&[&bob_id])
        .await
        .unwrap()
        .expect("User count not found");
    assert_eq!(alice_user_count.followers, 1);
    assert_eq!(alice_user_count.following, 1);
    assert_eq!(alice_user_count.friends, 1);

    let bob_user_count = UserCounts::try_from_index_json(&[&bob_id])
        .await
        .unwrap()
        .expect("User count not found");
    assert_eq!(bob_user_count.following, 1);
    assert_eq!(bob_user_count.following, 1);
    assert_eq!(bob_user_count.friends, 1);

    // Cleanup
    test.cleanup_user(&alice_id).await?;
    test.cleanup_user(&bob_id).await?;

    Ok(())
}
