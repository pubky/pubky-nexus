use crate::utils::watcher::WatcherTest;
use crate::{posts::utils::check_member_post_replies, users::utils::find_user_counts};
use anyhow::Result;
use nexus_common::{
    db::RedisOps,
    models::{
        post::{PostCounts, PostDetails, PostRelationships, PostView},
        user::UserCounts,
    },
};
use pubky::Keypair;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind, PubkyAppUser};

use super::utils::{
    check_member_global_timeline_user_post, check_member_total_engagement_user_posts,
    check_member_user_post_timeline,
};

#[tokio_shared_rt::test(shared)]
async fn test_delete_post_without_relationships() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create a new user
    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_delete_post_without_relationships".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostDelete:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    // Create a post without any relationships
    let post = PubkyAppPost {
        content: "Watcher:PostDelete:User:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };
    let post_id = test.create_post(&user_id, &post).await?;

    // Delete the post using the event handler
    test.cleanup_post(&user_id, &post_id).await?;

    // Attempt to find post details; should not exist in INDEX + GRAPH
    // Post:Details:user_id:post_id
    let post_details_result = PostDetails::get_by_id(&user_id, &post_id).await.unwrap();
    assert!(
        post_details_result.is_none(),
        "Post details should not be found after deletion"
    );

    // Attempt to find post counts; should not exist in INDEX + GRAPH
    let post_counts_result = PostCounts::get_by_id(&user_id, &post_id).await.unwrap();
    assert!(
        post_counts_result.is_none(),
        "Post counts should not be found after deletion"
    );

    // Attempt to get post view; should not exist
    let post_view = PostView::get_by_id(&user_id, &post_id, None, None, None)
        .await
        .unwrap();
    assert!(
        post_view.is_none(),
        "Post view should not be found after deletion"
    );

    let user_counts = UserCounts::get_by_id(&user_id)
        .await
        .unwrap()
        .expect("User counts should exist");
    assert_eq!(
        user_counts.posts, 0,
        "User count of posts should be again 0 after deletion"
    );

    let post_key = [user_id.as_str(), post_id.as_str()];

    // Post:Relationships:user_id:post_id
    let post_relationships = PostRelationships::try_from_index_json(&post_key, None)
        .await
        .unwrap();
    assert!(
        post_relationships.is_none(),
        "Post should not have any relationships"
    );

    // Assert the post does not belong to the global timeline
    // Sorted:Post:Global:Timeline
    let post_timeline = check_member_global_timeline_user_post(&user_id, &post_id)
        .await
        .unwrap();
    assert!(
        post_timeline.is_none(),
        "Post cannot exist in the global timeline, should be deleted"
    );

    // Assert the post does not belong to the global popularity
    // Sorted:Post:Global:TotalEngagement
    let post_engagement = check_member_total_engagement_user_posts(&post_key)
        .await
        .unwrap();
    assert!(
        post_engagement.is_none(),
        "Post cannot exist in the global total engagement, should be deleted"
    );

    // Assert that post does not belong to the user
    // Posts:User:user_id
    let user_post = check_member_user_post_timeline(&user_id, &post_id)
        .await
        .unwrap();
    assert!(
        user_post.is_none(),
        "Post cannot be linked to the user because it should not exist"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_delete_post_that_reposted() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create a new user
    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_delete_post_that_reposted".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostDeleteReposted:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    // Create a post without any relationships
    let post = PubkyAppPost {
        content: "Watcher:PostDeleteReposted:User:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };
    let post_id = test.create_post(&user_id, &post).await?;

    // Create a repost
    let repost = PubkyAppPost {
        content: "Watcher:PostDeleteReposted:User:RePost".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: Some(PubkyAppPostEmbed {
            kind: PubkyAppPostKind::Short,
            uri: format!("pubky://{user_id}/pub/pubky.app/posts/{post_id}"),
        }),
        attachments: None,
    };
    let repost_id = test.create_post(&user_id, &repost).await?;

    // Delete the post using the event handler
    test.cleanup_post(&user_id, &repost_id).await?;

    // GRAPH_OP + CACHE_OP: Assert relationship does not exist in the data layer

    // PARENT post counts should have reposts counts 0 once again
    let post_counts_result = PostCounts::get_by_id(&user_id, &post_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        post_counts_result.reposts, 0,
        "Original post reposts counts should be 0 after deletion of the repost"
    );

    // Attempt to find REPOST details; should not exist
    let post_details_result = PostDetails::get_by_id(&user_id, &repost_id).await.unwrap();
    assert!(
        post_details_result.is_none(),
        "Repost details should not be found after deletion"
    );

    // Attempt to find REPOST counts; should not exist
    let post_counts_result = PostCounts::get_by_id(&user_id, &repost_id).await.unwrap();
    assert!(
        post_counts_result.is_none(),
        "Repost counts should not be found after deletion"
    );

    // Attempt to get REPOST view; should not exist
    let post_view = PostView::get_by_id(&user_id, &repost_id, None, None, None)
        .await
        .unwrap();
    assert!(
        post_view.is_none(),
        "Repost view should not be found after deletion"
    );

    // CACHE_OP: Check if the event writes in the index
    // ########### PARENT RELATED INDEXES ################

    // Assert the parent post decrease in one the engagement score
    // Sorted:Post:Global:TotalEngagement
    let post_engagement = check_member_total_engagement_user_posts(&[&user_id, &post_id])
        .await
        .unwrap();
    assert!(
        post_engagement.is_some(),
        "Parent post should have global total engagement score, it seems that it does not exist"
    );
    assert_eq!(
        post_engagement.unwrap(),
        0,
        "Post engagement should decrease in one after repost deletion"
    );

    // Assert the parent user counts has changed stats
    // User:Counts:user_id:post_id
    let post_count = find_user_counts(&user_id).await;
    assert_eq!(post_count.posts, 1);

    // ########### REPLY RELATED INDEXES ################
    // Post:Relationships:user_id:post_id
    let post_relationships = PostRelationships::try_from_index_json(&[&user_id, &repost_id], None)
        .await
        .unwrap();
    assert!(
        post_relationships.is_none(),
        "Post should not have any relationships"
    );

    // Sorted:Posts:User:user_id
    // Check if the deleted repost does not exist
    let repost_timeline = check_member_user_post_timeline(&user_id, &repost_id)
        .await
        .unwrap();
    assert!(
        repost_timeline.is_none(),
        "Repost cannot exist in user timeline after deletion"
    );

    // Check if the deleted repost does not exist
    let repost_global_timeline = check_member_global_timeline_user_post(&user_id, &repost_id)
        .await
        .unwrap();
    assert!(
        repost_global_timeline.is_none(),
        "Repost cannot exist in global timeline after deletion"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_delete_post_that_replied() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create a new user
    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_delete_post_that_replied".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostDeleteReplied:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    // Create a post without any relationships
    let post = PubkyAppPost {
        content: "Watcher:PostDeleteReplied:User:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };
    let post_id = test.create_post(&user_id, &post).await?;

    // Create a reply
    let reply = PubkyAppPost {
        content: "Watcher:PostDeleteReplied:User:Reply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: Some(format!("pubky://{user_id}/pub/pubky.app/posts/{post_id}")),
        embed: None,
        attachments: None,
    };
    let reply_id = test.create_post(&user_id, &reply).await?;

    // Delete the post using the event handler
    test.cleanup_post(&user_id, &reply_id).await?;

    // GRAPH_OP + CACHE_OP: Assert relationship does not exist in the data layer
    // PARENT post counts should have reposts counts 0 once again
    let post_counts_result = PostCounts::get_by_id(&user_id, &post_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        post_counts_result.replies, 0,
        "Original post reposts counts should be 0 after deletion of the repost"
    );

    // Attempt to find post REPLY details; should not exist
    let post_details_result = PostDetails::get_by_id(&user_id, &reply_id).await.unwrap();
    assert!(
        post_details_result.is_none(),
        "Repost details should not be found after deletion"
    );

    // Attempt to find post REPLY counts; should not exist
    let post_counts_result = PostCounts::get_by_id(&user_id, &reply_id).await.unwrap();
    assert!(
        post_counts_result.is_none(),
        "Repost counts should not be found after deletion"
    );

    // Attempt to get post REPLY view; should not exist
    let post_view = PostView::get_by_id(&user_id, &reply_id, None, None, None)
        .await
        .unwrap();
    assert!(
        post_view.is_none(),
        "Repost view should not be found after deletion"
    );

    // CACHE_OP: Check if the event writes in the index
    // ########### PARENT RELATED INDEXES ################

    // Assert the parent post decrease in one the engagement score
    // Sorted:Post:Global:TotalEngagement
    let post_engagement = check_member_total_engagement_user_posts(&[&user_id, &post_id])
        .await
        .unwrap();
    assert!(
        post_engagement.is_some(),
        "Parent post should have global total engagement score, it seems that it does not exist"
    );
    assert_eq!(
        post_engagement.unwrap(),
        0,
        "Post engagement should decrease in one after reply deletion"
    );

    // Check if post reply was deleted from parent post replies list
    // Sorted:Posts:Replies:user_id:post_id
    let post_replies = check_member_post_replies(&user_id, &post_id, &[&user_id, &reply_id])
        .await
        .unwrap();
    assert!(
        post_replies.is_none(),
        "Reply id cannot exist in post replies"
    );

    // ########### REPLY RELATED INDEXES ################
    // Post:Relationships:user_id:post_id
    let post_relationships = PostRelationships::try_from_index_json(&[&user_id, &reply_id], None)
        .await
        .unwrap();
    assert!(
        post_relationships.is_none(),
        "Post should not have any relationships"
    );

    Ok(())
}
