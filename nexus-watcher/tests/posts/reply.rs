use super::utils::{
    check_member_global_timeline_user_post, check_member_total_engagement_user_posts,
    check_member_user_post_timeline, check_member_user_replies_timeline, find_post_counts,
    find_post_details, find_reply_relationship_parent_uri,
};
use crate::users::utils::find_user_counts;
use crate::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::{
    db::{kv::SortOrder, RedisOps},
    models::post::{PostDetails, PostRelationships, PostStream},
};
use pubky::Keypair;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostKind, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
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
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let parent_post_id = test.create_post(&user_id, &parent_post).await?;

    // Create reply uri
    let parent_uri = format!("pubky://{user_id}/pub/pubky.app/posts/{parent_post_id}");

    let reply_post = PubkyAppPost {
        content: "Watcher:PostReply:User:Reply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: Some(parent_uri.clone()),
        embed: None,
        attachments: None,
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

    // CACHE_OP: Check if the event writes in the index
    // ########### PARENT RELATED INDEXES ################
    // Sorted:Post:Replies:user_id:post_id
    let post_replies = PostStream::get_post_replies(
        &user_id,
        &parent_post_id,
        SortOrder::Descending,
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();
    assert_eq!(post_replies.len(), 1);
    let post_key = format!("{user_id}:{reply_id}");
    assert_eq!(post_replies[0], post_key);

    // Assert the parent post has changed stats, Post:Counts:user_id:post_id
    let post_count = find_post_counts(&user_id, &parent_post_id).await;
    assert_eq!(post_count.replies, 1);

    // Check if parent post engagement: Sorted:Posts:Global:TotalEngagement:user_id:post_id
    let total_engagement = check_member_total_engagement_user_posts(&[&user_id, &parent_post_id])
        .await
        .unwrap();
    assert!(total_engagement.is_some());
    assert_eq!(total_engagement.unwrap(), 1);

    // Assert the user has changed stats
    // User:Counts:user_id
    let user_count = find_user_counts(&user_id).await;
    assert_eq!(user_count.posts, 2);
    assert_eq!(user_count.replies, 1);

    // ########### REPLY RELATED INDEXES ################
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

    // Post:Relationships:user_id:post_id
    let post_relationships = PostRelationships::try_from_index_json(&[&user_id, &reply_id], None)
        .await
        .unwrap();
    assert!(
        post_relationships.is_some(),
        "Reply should have some relationship"
    );
    let relationships = post_relationships.unwrap();
    assert!(
        relationships.replied.is_some(),
        "Reply should have parent post URI"
    );
    assert_eq!(
        relationships.replied.unwrap(),
        parent_uri,
        "The parent URIs does not match"
    );

    // Sorted:Posts:AuthorParents:user_id
    // Check that replies are NOT in the user's timeline
    let user_timeline_timestamp = check_member_user_post_timeline(&user_id, &reply_id)
        .await
        .unwrap_or_default();
    assert!(
        user_timeline_timestamp.is_none(),
        "Replies should not be in the user's main timeline"
    );

    // Sorted:Posts:AuthorReplies:user_id
    // Check that replies are in the user's replies timeline
    let user_replies_timeline_timestamp = check_member_user_replies_timeline(&user_id, &reply_id)
        .await
        .unwrap_or_default();
    assert!(
        user_replies_timeline_timestamp.is_some(),
        "Replies should be in the user's main timeline"
    );

    // Check that replies are NOT in the global timeline
    let global_timeline_timestamp = check_member_global_timeline_user_post(&user_id, &reply_id)
        .await
        .unwrap_or_default();
    assert!(
        global_timeline_timestamp.is_none(),
        "Replies should not be in the global timeline"
    );

    // Check that replies are NOT in the global total engagement sorted set
    let reply_key = format!("{}:{}", user_id, &reply_id);
    let global_total_engagement = check_member_total_engagement_user_posts(&[&reply_key])
        .await
        .unwrap_or_default();
    assert!(
        global_total_engagement.is_none(),
        "Replies should not be in the global total engagement sorted set"
    );

    test.cleanup_post(&user_id, &reply_id).await?;

    // Cleanup
    test.cleanup_user(&user_id).await?;
    test.cleanup_post(&user_id, &parent_post_id).await?;

    Ok(())
}
