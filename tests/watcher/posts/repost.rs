use crate::watcher::utils::WatcherTest;
use anyhow::Result;
use pubky_common::crypto::Keypair;
use pubky_nexus::{
    models::{
        post::{
            PostCounts, PostStream, PostView, POST_PER_USER_KEY_PARTS,
            POST_TOTAL_ENGAGEMENT_KEY_PARTS,
        },
        pubky_app::{PostEmbed, PostKind, PubkyAppPost, PubkyAppUser},
        user::UserCounts,
    },
    RedisOps,
};

#[tokio::test]
async fn test_homeserver_repost() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Test reposter".to_string(),
        status: None,
    };

    let user_id = test.create_user(&keypair, &user).await?;

    let post_post = PubkyAppPost {
        content: "This is a test post!".to_string(),
        kind: PostKind::Short,
        parent: None,
        embed: None,
    };

    let post_id = test.create_post(&user_id, &post_post).await?;

    // Assert the new post can be served from Nexus
    let result_post = PostView::get_by_id(&user_id, &post_id, None, None, None)
        .await
        .unwrap()
        .expect("The new post was not served from Nexus");

    assert_eq!(result_post.details.id, post_id);
    assert_eq!(result_post.details.content, post_post.content);
    assert_eq!(
        result_post.details.uri,
        format!("pubky://{user_id}/pub/pubky.app/posts/{post_id}")
    );
    assert_eq!(result_post.counts.reposts, 0);
    assert!(result_post.details.indexed_at > 0);
    assert_eq!(result_post.counts.tags, 0);
    assert_eq!(result_post.counts.replies, 0);

    // Create repost
    let post_uri = format!("pubky://{}/pub/pubky.app/posts/{}", user_id, post_id);

    let repost = PubkyAppPost {
        content: "This is a repost post!".to_string(),
        kind: PostKind::Short,
        parent: None,
        embed: Some(PostEmbed {
            kind: PostKind::Short,
            uri: post_uri.clone(),
        }),
    };

    let repost_id = test.create_post(&user_id, &repost).await?;

    // Assert the new repost can be served from Nexus
    let result_repost = PostView::get_by_id(&user_id, &repost_id, None, None, None)
        .await
        .unwrap()
        .expect("The repost was not served from Nexus");

    assert_eq!(result_repost.details.id, repost_id);
    assert_eq!(result_repost.details.content, repost.content);
    assert_eq!(
        result_repost.details.uri,
        format!("pubky://{user_id}/pub/pubky.app/posts/{repost_id}")
    );
    assert_eq!(result_repost.counts.reposts, 0);
    assert!(result_repost.details.indexed_at > 0);
    assert_eq!(result_repost.counts.tags, 0);
    assert_eq!(result_repost.counts.replies, 0);
    assert_eq!(result_repost.relationships.reposted, Some(post_uri));

    let parent_post_key: [&str; 2] = [&user_id, &post_id];

    let post_count = PostCounts::try_from_index_json(&parent_post_key)
        .await
        .unwrap()
        .expect("The new post was not served from Nexus");

    assert_eq!(post_count.reposts, 1);

    // Check if parent post engagement: Sorted:Posts:Global:TotalEngagement:user_id:post_id
    let total_engagement =
        PostStream::check_sorted_set_member(&POST_TOTAL_ENGAGEMENT_KEY_PARTS, &parent_post_key)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(total_engagement, 1);

    // Sorted:Post:User:user_id
    let post_stream_key_parts = [&POST_PER_USER_KEY_PARTS[..], &[&user_id]].concat();
    let post_timeline = PostStream::check_sorted_set_member(&post_stream_key_parts, &[&repost_id])
        .await
        .unwrap();
    assert_eq!(post_timeline.is_some(), true);

    let exist_count = UserCounts::try_from_index_json(&[&user_id])
        .await
        .unwrap()
        .expect("User count not found");

    assert_eq!(exist_count.posts, 2);

    // // TODO: Impl DEL post. Assert the repost does not exist in Nexus
    test.cleanup_post(&user_id, &repost_id).await?;
    // let result_post = PostView::get_by_id(&user_id, &post_id, None, None, None)
    //     .await
    //     .unwrap();

    // assert!(result_post.is_none(), "The post should have been deleted");

    // Cleanup
    test.cleanup_user(&user_id).await?;
    test.cleanup_post(&user_id, &post_id).await?;

    Ok(())
}
