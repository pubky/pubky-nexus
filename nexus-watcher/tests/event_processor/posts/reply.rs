use super::utils::{
    check_member_global_timeline_user_post, check_member_total_engagement_user_posts,
    check_member_user_post_timeline, check_member_user_replies_timeline, find_post_counts,
    find_post_details, find_reply_relationship_parent_uri,
};
use crate::event_processor::users::utils::find_user_counts;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::{
    db::{kv::SortOrder, RedisOps},
    models::post::{PostCounts, PostDetails, PostRelationships, PostStream},
};
use pubky::Keypair;
use pubky_app_specs::{post_uri_builder, PubkyAppPost, PubkyAppPostKind, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_reply() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();

    let user = PubkyAppUser {
        bio: Some("test_homeserver_post_reply".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostReply:User".to_string(),
        status: None,
    };

    let user_id = test.create_user(&user_kp, &user).await?;

    let parent_post = PubkyAppPost {
        content: "Watcher:PostReply:User:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let (parent_post_id, parent_post_path) = test.create_post(&user_kp, &parent_post).await?;

    // Create reply uri
    let parent_absolute_uri = post_uri_builder(user_id.clone(), parent_post_id.clone());

    let reply_post = PubkyAppPost {
        content: "Watcher:PostReply:User:Reply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: Some(parent_absolute_uri.clone()),
        embed: None,
        attachments: None,
    };

    let (reply_id, reply_path) = test.create_post(&user_kp, &reply_post).await?;

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
    assert_eq!(reply_parent_uri, parent_absolute_uri);

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
    assert_eq!(post_replies.post_keys.len(), 1);
    let post_key = format!("{user_id}:{reply_id}");
    assert_eq!(post_replies.post_keys[0], post_key);
    assert!(post_replies.last_post_score.is_some());

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
        relationships.replied.unwrap().try_to_uri_str().unwrap(),
        parent_absolute_uri,
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

    test.cleanup_post(&user_kp, &reply_path).await?;

    // Cleanup
    test.cleanup_user(&user_kp).await?;
    test.cleanup_post(&user_kp, &parent_post_path).await?;

    Ok(())
}

/// Regression for the `!is_reply` inversion in `PostCounts::get_by_id`
/// (Greptile P1). The cache-miss code path rebuilds PostCounts from the
/// graph and writes it back via `put_to_index`. Previously it passed
/// `!is_reply` (double-negated), so the engagement-gate inside
/// `put_to_index` (`if !is_reply`) evaluated `true` for replies — adding
/// the reply to POST_TOTAL_ENGAGEMENT on every cache rebuild after a
/// Redis eviction.
///
/// Test plan: create a reply (which correctly stays out of
/// POST_TOTAL_ENGAGEMENT via the watcher's normal path), evict its
/// PostCounts JSON, call `get_by_id` to force the cache-miss rebuild, then
/// assert the reply is still absent from POST_TOTAL_ENGAGEMENT.
#[tokio_shared_rt::test(shared)]
async fn test_postcounts_get_by_id_does_not_leak_reply_into_engagement_on_cache_miss() -> Result<()>
{
    let mut test = WatcherTest::setup(None).await?;
    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_postcounts_get_by_id_cache_miss".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostCountsCacheMiss:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    // Parent post (kind=Short) — used only as a reply target.
    let parent_post = PubkyAppPost {
        content: "parent".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };
    let (parent_post_id, parent_post_path) = test.create_post(&user_kp, &parent_post).await?;
    let parent_uri = post_uri_builder(user_id.clone(), parent_post_id.clone());

    // Reply post — by construction NOT supposed to enter POST_TOTAL_ENGAGEMENT.
    let reply_post = PubkyAppPost {
        content: "reply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: Some(parent_uri),
        embed: None,
        attachments: None,
    };
    let (reply_id, reply_path) = test.create_post(&user_kp, &reply_post).await?;

    // Pre-condition: the watcher's normal path correctly gated the reply out.
    let reply_key: &[&str] = &[&user_id, &reply_id];
    assert!(
        check_member_total_engagement_user_posts(reply_key)
            .await?
            .is_none(),
        "pre-condition: reply must not be in POST_TOTAL_ENGAGEMENT after normal indexing"
    );

    // Evict the PostCounts JSON from Redis, forcing `get_by_id` to fall through
    // to the graph and re-populate the cache via `put_to_index`.
    PostCounts::remove_from_index_multiple_json(&[reply_key]).await?;
    assert!(
        PostCounts::get_from_index(&user_id, &reply_id)
            .await?
            .is_none(),
        "eviction precondition: PostCounts JSON should be gone"
    );

    // Trigger the cache-miss rebuild path that the bug lives on.
    let rebuilt = PostCounts::get_by_id(&user_id, &reply_id).await?;
    assert!(
        rebuilt.is_some(),
        "get_by_id must rebuild PostCounts from the graph after eviction"
    );

    // The load-bearing assertion: the rebuild must NOT have added the reply
    // to POST_TOTAL_ENGAGEMENT. With the `!is_reply` bug present, the next
    // line would see the reply with score 0 in the engagement sorted set.
    assert!(
        check_member_total_engagement_user_posts(reply_key)
            .await?
            .is_none(),
        "reply must not leak into POST_TOTAL_ENGAGEMENT on cache-miss rebuild"
    );

    test.cleanup_post(&user_kp, &reply_path).await?;
    test.cleanup_post(&user_kp, &parent_post_path).await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}
