use super::utils::{
    check_member_total_engagement_user_posts, check_member_user_post_timeline, find_post_counts,
    find_post_details, find_repost_relationship_parent_uri,
};
use crate::watcher::users::utils::find_user_counts;
use crate::watcher::utils::WatcherTest;
use anyhow::Result;
use pubky_common::crypto::Keypair;
use pubky_nexus::{
    models::{
        post::PostDetails,
        pubky_app::{PostEmbed, PostKind, PubkyAppPost, PubkyAppUser},
    },
    RedisOps,
};

#[tokio::test]
async fn test_homeserver_post_repost() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let keypair = Keypair::random();

    let user = PubkyAppUser {
        bio: Some("test_homeserver_post_repost".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostRepost:User".to_string(),
        status: None,
    };

    let user_id = test.create_user(&keypair, &user).await?;

    let parent_post = PubkyAppPost {
        content: "Watcher:PostRepost:User:Post".to_string(),
        kind: PostKind::Short,
        parent: None,
        embed: None,
    };

    let parent_post_id = test.create_post(&user_id, &parent_post).await?;

    // Create repost uri
    let parent_uri = format!("pubky://{user_id}/pub/pubky.app/posts/{parent_post_id}");

    let repost = PubkyAppPost {
        content: "Watcher:PostReply:User:Repost".to_string(),
        kind: PostKind::Short,
        parent: None,
        embed: Some(PostEmbed {
            kind: PostKind::Short,
            uri: parent_uri.clone(),
        }),
    };

    let repost_id = test.create_post(&user_id, &repost).await?;

    // GRAPH_OP: Assert repost relationship was created
    let repost_post_details = find_post_details(&user_id, &repost_id).await.unwrap();

    assert_eq!(repost_post_details.id, repost_id);
    assert_eq!(repost_post_details.content, repost.content);
    assert_eq!(
        repost_post_details.uri,
        format!("pubky://{user_id}/pub/pubky.app/posts/{repost_id}")
    );
    assert!(repost_post_details.indexed_at > 0);

    // Assert post reply relationship
    let reply_parent_uri = find_repost_relationship_parent_uri(&user_id, &repost_id)
        .await
        .unwrap();
    assert_eq!(reply_parent_uri, parent_uri);

    // CACHE_OP: Check if the event writes in the graph
    let repost_post_key: [&str; 2] = [&user_id, &repost_id];

    //User:Details:user_id:post_id
    let post_detail_cache: PostDetails = PostDetails::try_from_index_json(&repost_post_key)
        .await
        .unwrap()
        .expect("The new post detail was not served from Nexus cache");

    assert_eq!(repost_post_details.id, post_detail_cache.id);
    assert_eq!(repost_post_details.content, post_detail_cache.content);
    assert_eq!(repost_post_details.uri, post_detail_cache.uri);
    assert_eq!(repost_post_details.indexed_at, post_detail_cache.indexed_at);

    // User:Counts:user_id:post_id
    let reply_post_counts = find_post_counts(&repost_post_key).await;

    assert_eq!(reply_post_counts.reposts, 0);
    assert_eq!(reply_post_counts.tags, 0);
    assert_eq!(reply_post_counts.replies, 0);

    let parent_post_key: [&str; 2] = [&user_id, &parent_post_id];
    // Assert the parent post has changed stats, User:Counts:user_id:post_id
    let post_count = find_post_counts(&parent_post_key).await;
    assert_eq!(post_count.reposts, 1);

    // Check if parent post engagement: Sorted:Posts:Global:TotalEngagement:user_id:post_id
    let total_engagement = check_member_total_engagement_user_posts(&parent_post_key)
        .await
        .unwrap();
    assert_eq!(total_engagement.is_some(), true);
    assert_eq!(total_engagement.unwrap(), 1);

    // Sorted:Posts:User:user_id
    let post_timeline = check_member_user_post_timeline(&user_id, &repost_id)
        .await
        .unwrap();
    assert_eq!(post_timeline.is_some(), true);
    assert_eq!(
        post_timeline.unwrap(),
        repost_post_details.indexed_at as isize
    );

    // Assert the parent post has changed stats
    // User:Counts:user_id:post_id
    let post_count = find_user_counts(&user_id).await;
    assert_eq!(post_count.posts, 2);

    // // TODO: Impl DEL post. Assert the repost does not exist in Nexus
    test.cleanup_post(&user_id, &repost_id).await?;
    // let result_post = PostView::get_by_id(&user_id, &post_id, None, None, None)
    //     .await
    //     .unwrap();

    // assert!(result_post.is_none(), "The post should have been deleted");

    // Cleanup
    test.cleanup_user(&user_id).await?;
    test.cleanup_post(&user_id, &parent_post_id).await?;

    Ok(())
}
