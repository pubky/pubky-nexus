use super::utils::{
    check_member_global_timeline_user_post, check_member_total_engagement_user_posts,
    check_member_user_post_timeline, find_post_counts, find_post_details,
    find_repost_relationship_parent_uri,
};
use crate::event_processor::users::utils::find_user_counts;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::{
    db::RedisOps,
    models::post::{PostDetails, PostRelationships},
};
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder, PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind, PubkyAppUser,
};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_repost() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();

    let user = PubkyAppUser {
        bio: Some("test_homeserver_post_repost".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostRepost:User".to_string(),
        status: None,
    };

    let user_id = test.create_user(&user_kp, &user).await?;

    let parent_post = PubkyAppPost {
        content: "Watcher:PostRepost:User:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let parent_post_id = test.create_post(&user_kp, &parent_post).await?;

    // Create repost uri
    let parent_absolute_uri = post_uri_builder(user_id.clone(), parent_post_id.clone());

    let repost = PubkyAppPost {
        content: "Watcher:PostReply:User:Repost".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: Some(PubkyAppPostEmbed {
            kind: PubkyAppPostKind::Short,
            uri: parent_absolute_uri.clone(),
        }),
        attachments: None,
    };

    let repost_id = test.create_post(&user_kp, &repost).await?;

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
    assert_eq!(reply_parent_uri, parent_absolute_uri);

    // CACHE_OP: Check if the event writes in the index

    // ########### PARENT RELATED INDEXES ################
    // Assert the parent post has changed stats, User:Counts:user_id:post_id
    let post_count = find_post_counts(&user_id, &parent_post_id).await;
    assert_eq!(post_count.reposts, 1);

    // Assert the parent user counts has changed stats
    // User:Counts:user_id:post_id
    let post_count = find_user_counts(&user_id).await;
    assert_eq!(post_count.posts, 2);

    // Check if parent post engagement: Sorted:Posts:Global:TotalEngagement:user_id:post_id
    let parent_total_engagement =
        check_member_total_engagement_user_posts(&[&user_id, &parent_post_id])
            .await
            .unwrap();
    assert!(
        parent_total_engagement.is_some(),
        "Parent post total engagement should be increased by one"
    );
    assert_eq!(parent_total_engagement.unwrap(), 1);

    // ########### REPOST RELATED INDEXES ################
    //User:Details:user_id:post_id
    let post_detail_cache: PostDetails = PostDetails::get_from_index(&user_id, &repost_id)
        .await
        .unwrap()
        .expect("The new post detail was not served from Nexus cache");

    assert_eq!(repost_post_details.id, post_detail_cache.id);
    assert_eq!(repost_post_details.content, post_detail_cache.content);
    assert_eq!(repost_post_details.uri, post_detail_cache.uri);
    assert_eq!(repost_post_details.indexed_at, post_detail_cache.indexed_at);

    // Post:Relationships:user_id:post_id
    let post_relationships = PostRelationships::try_from_index_json(&[&user_id, &repost_id], None)
        .await
        .unwrap();
    assert!(
        post_relationships.is_some(),
        "Repost should have some relationship"
    );
    let relationships = post_relationships.unwrap();
    assert!(
        relationships.reposted.is_some(),
        "Repost should have parent post URI"
    );
    assert_eq!(
        relationships.reposted.unwrap(),
        parent_absolute_uri,
        "The parent URIs does not match"
    );

    // User:Counts:user_id:post_id
    let reply_post_counts = find_post_counts(&user_id, &repost_id).await;

    assert_eq!(reply_post_counts.reposts, 0);
    assert_eq!(reply_post_counts.tags, 0);
    assert_eq!(reply_post_counts.replies, 0);

    // Sorted:Posts:User:user_id
    // Check that repost is in the user's timeline
    let repost_timeline = check_member_user_post_timeline(&user_id, &repost_id)
        .await
        .unwrap();
    assert!(repost_timeline.is_some());
    assert_eq!(
        repost_timeline.unwrap(),
        repost_post_details.indexed_at as isize
    );

    // Check that repost is in the global timeline
    let repost_global_timeline_timestamp =
        check_member_global_timeline_user_post(&user_id, &repost_id)
            .await
            .unwrap();
    assert!(
        repost_global_timeline_timestamp.is_some(),
        "Repost should be in the global timeline"
    );
    assert_eq!(
        repost_global_timeline_timestamp.unwrap(),
        repost_post_details.indexed_at as isize
    );

    // Check that repost is in the global total engagement sorted set
    let repost_key = format!("{}:{}", user_id, &repost_id);

    let repost_global_total_engagement = check_member_total_engagement_user_posts(&[&repost_key])
        .await
        .unwrap_or_default();
    assert!(
        repost_global_total_engagement.is_some(),
        "Repost should be in the global total engagement sorted set"
    );

    // // TODO: Impl DEL post. Assert the repost does not exist in Nexus
    test.cleanup_post(&user_kp, &repost_id).await?;
    // let result_post = PostView::get_by_id(&user_id, &post_id, None, None, None)
    //     .await
    //     .unwrap();

    // assert!(result_post.is_none(), "The post should have been deleted");

    // Cleanup
    test.cleanup_user(&user_kp).await?;
    test.cleanup_post(&user_kp, &parent_post_id).await?;

    Ok(())
}
