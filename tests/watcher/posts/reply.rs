use super::utils::{
    check_member_total_engagement_user_posts, check_member_user_post_timeline, find_post_counts,
    find_post_details, find_reply_relationship_parent_uri,
};
use crate::watcher::{users::utils::find_user_counts, utils::WatcherTest};
use anyhow::Result;
use pubky_common::crypto::Keypair;
use pubky_nexus::models::{
    post::{PostDetails, PostThread},
    pubky_app::{PostKind, PubkyAppPost, PubkyAppUser},
};

#[tokio::test]
async fn test_homeserver_post_reply() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let keypair = Keypair::random();

    let user = PubkyAppUser {
        bio: Some("test_homeserver_post_reply".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostReply:User".to_string(),
        status: None,
    };

    let user_id = test.create_user(&keypair, &user).await?;

    let parent_post = PubkyAppPost {
        content: "Watcher:PostReply:User:Post".to_string(),
        kind: PostKind::Short,
        parent: None,
        embed: None,
    };

    let parent_post_id = test.create_post(&user_id, &parent_post).await?;

    // Create reply uri
    let parent_uri = format!("pubky://{user_id}/pub/pubky.app/posts/{parent_post_id}");

    let reply_post = PubkyAppPost {
        content: "Watcher:PostReply:User:Reply".to_string(),
        kind: PostKind::Short,
        parent: Some(parent_uri.clone()),
        embed: None,
    };

    let reply_id = test.create_post(&user_id, &reply_post).await?;

    // GRAPH_OP: Assert reply relationship was created
    let reply_post_details = find_post_details(&user_id, &reply_id).await.unwrap();

    assert_eq!(reply_post_details.id, reply_id);
    assert_eq!(reply_post_details.content, reply_post.content);
    assert_eq!(
        reply_post_details.uri,
        format!("pubky://{user_id}/pub/pubky.app/posts/{reply_id}")
    );
    assert!(reply_post_details.indexed_at > 0);

    // Assert post reply relationship
    let reply_parent_uri = find_reply_relationship_parent_uri(&user_id, &reply_id)
        .await
        .unwrap();
    assert_eq!(reply_parent_uri, parent_uri);

    // CACHE_OP: Check if the event writes in the graph
    //User:Details:user_id:post_id
    let post_detail_cache: PostDetails = PostDetails::get_from_index(&user_id, &reply_id)
        .await
        .unwrap()
        .expect("The new post detail was not served from Nexus cache");

    assert_eq!(reply_post_details.id, post_detail_cache.id);
    assert_eq!(reply_post_details.content, post_detail_cache.content);
    assert_eq!(reply_post_details.uri, post_detail_cache.uri);
    assert_eq!(reply_post_details.indexed_at, post_detail_cache.indexed_at);

    // User:Counts:user_id:post_id
    let reply_post_counts = find_post_counts(&user_id, &reply_id).await;

    assert_eq!(reply_post_counts.reposts, 0);
    assert_eq!(reply_post_counts.tags, 0);
    assert_eq!(reply_post_counts.replies, 0);

    // Assert the parent post has changed stats, User:Counts:user_id:post_id
    let post_count = find_post_counts(&user_id, &parent_post_id).await;
    assert_eq!(post_count.replies, 1);

    // Check if parent post engagement: Sorted:Posts:Global:TotalEngagement:user_id:post_id
    let total_engagement = check_member_total_engagement_user_posts(&[&user_id, &parent_post_id])
        .await
        .unwrap();
    assert!(total_engagement.is_some());
    assert_eq!(total_engagement.unwrap(), 1);

    // Sorted:Posts:User:user_id
    let post_timeline = check_member_user_post_timeline(&user_id, &reply_id)
        .await
        .unwrap();
    assert!(post_timeline.is_some());
    assert_eq!(
        post_timeline.unwrap(),
        reply_post_details.indexed_at as isize
    );

    // Assert the parent post has changed stats
    // User:Counts:user_id:post_id
    let post_count = find_user_counts(&user_id).await;
    assert_eq!(post_count.posts, 2);

    // GRAPH_OP: Fetch the post thread and confirm the reply is present
    let thread = PostThread::get_by_id(&user_id, &parent_post_id, None, 0, 10)
        .await
        .expect("Failed to fetch post thread")
        .expect("The post thread should exist");

    assert_eq!(thread.root_post.details.id, parent_post_id);
    assert_eq!(thread.replies.len(), 1);
    assert_eq!(thread.replies[0].details.id, reply_id);
    assert_eq!(thread.replies[0].details.content, reply_post.content);

    // // TODO: Impl DEL post. Assert the reply does not exist in Nexus
    test.cleanup_post(&user_id, &reply_id).await?;
    // let result_post = PostView::get_by_id(&user_id, &post_id, None, None, None)
    //     .await
    //     .unwrap();

    // assert!(result_post.is_none(), "The post should have been deleted");

    // After deletion, fetch the post thread again and confirm the reply is gone
    // let thread_after_deletion = PostThread::get_by_id(&user_id, &parent_id, None, 0, 10)
    //     .await
    //     .expect("Failed to fetch post thread after deletion")
    //     .expect("The post thread should exist after deletion");

    // Cleanup
    test.cleanup_user(&user_id).await?;
    test.cleanup_post(&user_id, &parent_post_id).await?;

    Ok(())
}
