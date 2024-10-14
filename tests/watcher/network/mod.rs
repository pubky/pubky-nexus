use crate::watcher::utils::WatcherTest;
use anyhow::Result;
use pubky_common::crypto::Keypair;
use pubky_nexus::{
    models::{
        pubky_app::{traits::HashId, PostKind, PubkyAppPost, PubkyAppTag, PubkyAppUser},
        tag::{traits::TagCollection, user::TagUser},
        user::{Followers, Following, UserCounts, UserFollows, UserView},
    },
    RedisOps,
};

#[tokio::test]
async fn test_network_scenario() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create users: Alice, Bob, and Charlie
    let user_names = vec!["Alice", "Bob", "Charlie"];
    let mut user_ids = Vec::new();
    let mut keypairs = Vec::new();

    for name in user_names.iter() {
        let keypair = Keypair::random();
        let user = PubkyAppUser {
            bio: Some(format!("{}'s bio", name)),
            image: None,
            links: None,
            name: name.to_string(),
            status: None,
        };
        let user_id = test.create_user(&keypair, &user).await?;
        user_ids.push(user_id);
        keypairs.push(keypair);
    }

    // Users create posts
    let mut post_ids = Vec::new(); // Vec of Vecs to store posts per user
    for (i, user_id) in user_ids.iter().enumerate() {
        let user_posts = vec![
            PubkyAppPost {
                content: format!("{}'s first post", user_names[i]),
                kind: PostKind::Short,
                parent: None,
                embed: None,
                attachments: None,
            },
            PubkyAppPost {
                content: format!("{}'s second post", user_names[i]),
                kind: PostKind::Short,
                parent: None,
                embed: None,
                attachments: None,
            },
        ];
        let mut user_post_ids = Vec::new();
        for post in user_posts {
            let post_id = test.create_post(&user_id, &post).await?;
            user_post_ids.push(post_id);
        }
        post_ids.push(user_post_ids);
    }

    // Users tag posts
    // Alice tags Bob's first post
    let alice_id = &user_ids[0];
    let bob_id = &user_ids[1];
    let bob_first_post_id = &post_ids[1][0];
    let tag_label = "interesting";
    let tag = PubkyAppTag {
        uri: format!(
            "pubky://{}/pub/pubky.app/posts/{}",
            bob_id, bob_first_post_id
        ),
        label: tag_label.to_string(),
        created_at: chrono::Utc::now().timestamp_millis(),
    };

    let tag_url = format!(
        "pubky://{}/pub/pubky.app/tags/{}",
        alice_id,
        tag.create_id()
    );

    test.create_tag(&tag_url, serde_json::to_vec(&tag)?).await?;

    // Users tag other users
    // Bob tags Charlie
    let charlie_id = &user_ids[2];
    let tag_label_user = "friend";
    let tag_user = PubkyAppTag {
        uri: format!("pubky://{}/pub/pubky.app/profile.json", charlie_id),
        label: tag_label_user.to_string(),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let tag_url_user = format!(
        "pubky://{}/pub/pubky.app/tags/{}",
        bob_id,
        tag_user.create_id()
    );

    test.create_tag(&tag_url_user, serde_json::to_vec(&tag_user)?)
        .await?;

    // Users delete tags
    // Alice deletes the tag she made on Bob's post
    test.delete_tag(&tag_url).await?;

    // Users follow each other
    // Alice follows Bob and Charlie
    let follow_uri = test.create_follow(alice_id, bob_id).await?;
    test.create_follow(alice_id, charlie_id).await?;

    // Bob follows Alice
    test.create_follow(bob_id, alice_id).await?;

    // Users unfollow each other
    // Alice unfollows Bob
    test.delete_follow(&follow_uri).await?;

    // Assertions to ensure total counts are accurate and consistent between index and graph

    // Function to compare counts from cache and graph
    async fn compare_user_counts(user_id: &str) -> Result<UserCounts> {
        let counts_cache = UserCounts::try_from_index_json(&[user_id])
            .await
            .unwrap()
            .expect("Counts not found in cache");
        let counts_graph = UserCounts::get_from_graph(user_id)
            .await
            .unwrap()
            .expect("Counts not found in graph");
        assert_eq!(
            counts_cache.followers, counts_graph.followers,
            "Follower counts mismatch between cache and graph"
        );
        Ok(counts_graph)
    }

    // For Alice
    let alice_counts = compare_user_counts(alice_id).await?;
    // Alice has 2 posts, 1 following (Charlie), 1 follower (Bob), 0 friends (since she unfollowed Bob)
    assert_eq!(alice_counts.posts, 2, "Alice's post count mismatch");
    assert_eq!(
        alice_counts.following, 1,
        "Alice's following count mismatch"
    );
    assert_eq!(
        alice_counts.followers, 1,
        "Alice's followers count mismatch"
    );
    assert_eq!(alice_counts.friends, 0, "Alice's friends count mismatch");

    // For Bob
    let bob_counts = compare_user_counts(bob_id).await?;
    // Bob has 2 posts, 1 following (Alice), 0 followers (since Alice unfollowed him), 0 friends
    assert_eq!(bob_counts.posts, 2, "Bob's post count mismatch");
    assert_eq!(bob_counts.following, 1, "Bob's following count mismatch");
    assert_eq!(bob_counts.followers, 0, "Bob's followers count mismatch");
    assert_eq!(bob_counts.friends, 0, "Bob's friends count mismatch");

    // For Charlie
    let charlie_counts = compare_user_counts(charlie_id).await?;
    // Charlie has 2 posts, 0 following, 1 follower (Alice), 0 friends
    assert_eq!(charlie_counts.posts, 2, "Charlie's post count mismatch");
    assert_eq!(
        charlie_counts.following, 0,
        "Charlie's following count mismatch"
    );
    assert_eq!(
        charlie_counts.followers, 1,
        "Charlie's followers count mismatch"
    );
    assert_eq!(
        charlie_counts.friends, 0,
        "Charlie's friends count mismatch"
    );

    // Ensure that the tag counts are accurate
    // Bob's first post should have 0 tags (since Alice deleted her tag)
    let _bob_first_post_counts =
        pubky_nexus::models::post::PostCounts::get_by_id(bob_id, bob_first_post_id)
            .await
            .unwrap()
            .expect("Bob's first post counts not found");

    //TODO: Tag deleted from post
    // assert_eq!(
    //     bob_first_post_counts.tags, 0,
    //     "Bob's first post tag count mismatch"
    // );

    // Charlie's profile should have 1 tag (from Bob)
    let charlie = UserView::get_by_id(&charlie_id, None)
        .await
        .unwrap()
        .expect("Charlie's profile not found");
    assert_eq!(charlie.counts.tagged, 1, "Charlie's tag count mismatch");

    // Verify follow relationships
    // Alice is following Charlie
    let alice_following = Following::get_by_id(alice_id, None, None)
        .await
        .unwrap()
        .expect("Alice's following not found");
    assert_eq!(
        alice_following.0,
        vec![charlie_id.clone()],
        "Alice's following list mismatch"
    );

    // Alice's followers should be Bob
    let alice_followers = Followers::get_by_id(alice_id, None, None)
        .await
        .unwrap()
        .expect("Alice's followers not found");
    assert_eq!(
        alice_followers.0,
        vec![bob_id.clone()],
        "Alice's followers list mismatch"
    );

    // Bob's following is Alice
    let bob_following = Following::get_by_id(bob_id, None, None)
        .await
        .unwrap()
        .expect("Bob's following not found");
    assert_eq!(
        bob_following.0,
        vec![alice_id.clone()],
        "Bob's following list mismatch"
    );

    // Bob's followers should be none (since Alice unfollowed him)
    let bob_followers = Followers::get_by_id(bob_id, None, None)
        .await
        .unwrap()
        .expect("Bob's followers not found");
    assert!(bob_followers.0.is_empty(), "Bob should have no followers");

    // Clean up
    for user_id in user_ids {
        test.cleanup_user(&user_id).await?;
    }

    Ok(())
}
