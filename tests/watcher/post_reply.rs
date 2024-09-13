use super::utils::WatcherTest;
use anyhow::Result;
use pkarr::Keypair;
use pubky_nexus::{
    models::{
        post::{
            PostCounts, PostStream, PostThread, PostView, POST_PER_USER_KEY_PARTS,
            POST_TOTAL_ENGAGEMENT_KEY_PARTS,
        },
        pubky_app::{PostKind, PubkyAppPost, PubkyAppUser},
        user::UserCounts,
    },
    RedisOps,
};

#[tokio::test]
async fn test_homeserver_reply() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Test replyer".to_string(),
        status: None,
    };

    let user_id = test.create_user(&keypair, &user).await?;

    let parent_post = PubkyAppPost {
        content: "This is a test post!".to_string(),
        kind: PostKind::Short,
        parent: None,
        embed: None,
    };

    let parent_id = test.create_post(&user_id, &parent_post).await?;

    // Assert the new post can be served from Nexus
    let result_parent = PostView::get_by_id(&user_id, &parent_id, None, None, None)
        .await
        .unwrap()
        .expect("The new post was not served from Nexus");

    println!("New post served Parent: {:?}", result_parent.details);
    assert_eq!(result_parent.details.id, parent_id);
    assert_eq!(result_parent.details.content, parent_post.content);
    assert_eq!(
        result_parent.details.uri,
        format!("pubky://{user_id}/pub/pubky.app/posts/{parent_id}")
    );
    assert_eq!(result_parent.counts.reposts, 0);
    assert!(result_parent.details.indexed_at > 0);
    assert_eq!(result_parent.counts.tags, 0);
    assert_eq!(result_parent.counts.replies, 0);

    // Create reply
    let parent_uri = format!("pubky://{}/pub/pubky.app/posts/{}", user_id, parent_id);

    let reply = PubkyAppPost {
        content: "This is a reply post!".to_string(),
        kind: PostKind::Short,
        parent: Some(parent_uri.clone()),
        embed: None,
    };

    let reply_id = test.create_post(&user_id, &reply).await?;

    // Assert the new reply can be served from Nexus
    let result_reply = PostView::get_by_id(&user_id, &reply_id, None, None, None)
        .await
        .unwrap()
        .expect("The reply was not served from Nexus");

    println!(
        "New reply served Relationships: {:?}",
        result_reply.relationships
    );
    assert_eq!(result_reply.details.id, reply_id);
    assert_eq!(result_reply.details.content, reply.content);
    assert_eq!(
        result_reply.details.uri,
        format!("pubky://{user_id}/pub/pubky.app/posts/{reply_id}")
    );
    assert_eq!(result_reply.counts.reposts, 0);
    assert!(result_reply.details.indexed_at > 0);
    assert_eq!(result_reply.counts.tags, 0);
    assert_eq!(result_reply.counts.replies, 0);
    assert_eq!(result_reply.relationships.replied, Some(parent_uri));

    let parent_post_key: [&str; 2] = [&user_id, &parent_id];

    // Assert the parent post has changed stats
    let post_count = PostCounts::try_from_index_json(&parent_post_key)
        .await
        .unwrap()
        .expect("The new post was not served from Nexus");

    assert_eq!(post_count.replies, 1);

    // Check if parent post engagement: Sorted:Posts:Global:TotalEngagement:user_id:post_id
    let total_engagement =
        PostStream::check_sorted_set_member(&POST_TOTAL_ENGAGEMENT_KEY_PARTS, &parent_post_key)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(total_engagement, 1);

    // Sorted:Post:User:user_id
    let post_stream_key_parts = [&POST_PER_USER_KEY_PARTS[..], &[&user_id]].concat();
    let post_timeline = PostStream::check_sorted_set_member(&post_stream_key_parts, &[&parent_id])
        .await
        .unwrap();
    assert_eq!(post_timeline.is_some(), true);

    let exist_count = UserCounts::try_from_index_json(&[&user_id])
        .await
        .unwrap()
        .expect("User count not found");

    assert_eq!(exist_count.posts, 2);

    // Fetch the post thread and confirm the reply is present
    let thread = PostThread::get_by_id(&user_id, &parent_id, None, 0, 10)
        .await
        .expect("Failed to fetch post thread")
        .expect("The post thread should exist");

    assert_eq!(thread.root_post.details.id, parent_id);
    assert_eq!(thread.replies.len(), 1);
    assert_eq!(thread.replies[0].details.id, reply_id);
    assert_eq!(thread.replies[0].details.content, reply.content);

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
    test.cleanup_post(&user_id, &parent_id).await?;

    Ok(())
}
