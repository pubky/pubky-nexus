use super::utils::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use pkarr::Keypair;
use pubky_nexus::{
    models::{
        post::{PostCounts, PostStream, POST_TOTAL_ENGAGEMENT_KEY_PARTS},
        pubky_app::{PostEmbed, PostKind, PubkyAppFollow, PubkyAppPost, PubkyAppUser},
        user::{UserCounts, UserStream, USER_PIONEERS_KEY_PARTS},
    },
    RedisOps,
};

#[tokio::test]
async fn test_homeserver_post_pioneer() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let popular_user_keypair = Keypair::random();

    let popular_user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Wizard Developer".to_string(),
        status: None,
    };
    let popular_user_id = test
        .create_user(&popular_user_keypair, &popular_user)
        .await?;

    // Popular user creates a new post
    let popular_post = PubkyAppPost {
        content: "Here it is! Selfie-Records DNS TXT records".to_string(),
        kind: PostKind::Short,
        parent: None,
        embed: None,
    };

    let popular_post_id = test.create_post(&popular_user_id, &popular_post).await?;

    // Check pioneers score: Sorted:Users:Pioneers
    let pioneer_score =
        UserStream::check_sorted_set_member(&USER_PIONEERS_KEY_PARTS, &[&popular_user_id])
            .await
            .unwrap()
            .unwrap();
    assert_eq!(pioneer_score, 0);

    // Now unknown user, creates an account
    let unknown_user_keypair = Keypair::random();
    let unknown_user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "antonym".to_string(),
        status: None,
    };
    let unknown_user_id = test
        .create_user(&unknown_user_keypair, &unknown_user)
        .await?;

    // and start following the popular user
    let follow = PubkyAppFollow {
        created_at: Utc::now().timestamp_millis(),
    };
    let blob = serde_json::to_vec(&follow)?;
    let follow_url = format!(
        "pubky://{}/pub/pubky.app/follows/{}",
        unknown_user_id, popular_user_id
    );
    test.client.put(follow_url.as_str(), &blob).await?;

    // Process the event
    test.ensure_event_processing_complete().await?;

    // Check pioneers score: Sorted:Users:Pioneers
    let pioneer_score =
        UserStream::check_sorted_set_member(&USER_PIONEERS_KEY_PARTS, &[&popular_user_id])
            .await
            .unwrap()
            .unwrap();
    assert_eq!(pioneer_score, 1);

    // Create reply to popular user post
    let parent_uri = format!(
        "pubky://{}/pub/pubky.app/posts/{}",
        popular_user_id, popular_post_id
    );

    let reply = PubkyAppPost {
        content: "This really interesting the project".to_string(),
        kind: PostKind::Short,
        parent: Some(parent_uri.clone()),
        embed: None,
    };

    let _reply_id = test.create_post(&unknown_user_id, &reply).await?;

    // Create repost of popular user
    let post_uri = format!(
        "pubky://{}/pub/pubky.app/posts/{}",
        popular_user_id, popular_post_id
    );

    let repost = PubkyAppPost {
        content: "use it for PGP!".to_string(),
        kind: PostKind::Short,
        parent: None,
        embed: Some(PostEmbed {
            kind: PostKind::Short,
            uri: post_uri.clone(),
        }),
    };

    test.create_post(&unknown_user_id, &repost).await?;

    // Check if the interaction is cached
    let parent_post_key: [&str; 2] = [&popular_user_id, &popular_post_id];

    // Check if parent post engagement: Sorted:Posts:Global:TotalEngagement:user_id:post_id
    let total_engagement =
        PostStream::check_sorted_set_member(&POST_TOTAL_ENGAGEMENT_KEY_PARTS, &parent_post_key)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(total_engagement, 2);

    // Assert the parent post has changed stats. Post:Counts:user_id:post_id
    let post_count = PostCounts::try_from_index_json(&parent_post_key)
        .await
        .unwrap()
        .expect("The new post was not served from Nexus");

    assert_eq!(post_count.replies, 1);
    assert_eq!(post_count.reposts, 1);

    let popular_user_count = UserCounts::try_from_index_json(&[&popular_user_id])
        .await
        .unwrap()
        .expect("User count not found");
    assert_eq!(popular_user_count.posts, 1);

    let unknown_user_count = UserCounts::try_from_index_json(&[&unknown_user_id])
        .await
        .unwrap()
        .expect("User count not found");
    assert_eq!(unknown_user_count.posts, 2);

    // Check pioneers score: Sorted:Users:Pioneers
    let pioneer_score =
        UserStream::check_sorted_set_member(&USER_PIONEERS_KEY_PARTS, &[&popular_user_id])
            .await
            .unwrap()
            .unwrap();
    // Pioneer score does not update because popular user does not have any interaction
    assert_eq!(pioneer_score, 1);

    // // // TODO: Impl DEL post. Assert the reply does not exist in Nexus
    // test.cleanup_post(&user_id, &reply_id).await?;

    // Cleanup
    test.cleanup_user(&popular_user_id).await?;
    test.cleanup_user(&unknown_user_id).await?;
    //test.cleanup_post(&user_id, &parent_post_id).await?;

    Ok(())
}
