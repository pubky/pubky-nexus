use crate::watcher::utils::WatcherTest;
use anyhow::Result;
use pubky_common::crypto::Keypair;
use pubky_nexus::models::{
    post::{PostCounts, PostDetails, PostView},
    pubky_app::{PostEmbed, PostKind, PubkyAppPost, PubkyAppUser},
    user::UserCounts,
};

#[tokio::test]
async fn test_delete_post_without_relationships() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create a new user
    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("Test user for post deletion".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostDelete:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    // Create a post without any relationships
    let post = PubkyAppPost {
        content: "User's post to be deleted".to_string(),
        kind: PostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };
    let post_id = test.create_post(&user_id, &post).await?;

    // Delete the post using the event handler
    test.cleanup_post(&user_id, &post_id).await?;

    // Attempt to find post details; should not exist
    let post_details_result = PostDetails::get_by_id(&user_id, &post_id).await.unwrap();
    assert!(
        post_details_result.is_none(),
        "Post details should not be found after deletion"
    );

    // Attempt to find post counts; should not exist
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

    Ok(())
}

#[tokio::test]
async fn test_delete_post_that_reposted() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create a new user
    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("Test user for post deletion".to_string()),
        image: None,
        links: None,
        name: "UserForPostDeletion".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    // Create a post without any relationships
    let post = PubkyAppPost {
        content: "User's post to be deleted".to_string(),
        kind: PostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };
    let post_id = test.create_post(&user_id, &post).await?;

    // Create a repost
    let repost = PubkyAppPost {
        content: "User's post to be deleted".to_string(),
        kind: PostKind::Short,
        parent: None,
        embed: Some(PostEmbed {
            kind: PostKind::Short,
            uri: format!("pubky://{}/pub/pubky.app/posts/{}", user_id, post_id),
        }),
        attachments: None,
    };
    let repost_id = test.create_post(&user_id, &repost).await?;

    // Delete the post using the event handler
    test.cleanup_post(&user_id, &repost_id).await?;

    // Attempt to find post details; should not exist
    let post_details_result = PostDetails::get_by_id(&user_id, &repost_id).await.unwrap();
    assert!(
        post_details_result.is_none(),
        "Repost details should not be found after deletion"
    );

    // Attempt to find post counts; should not exist
    let post_counts_result = PostCounts::get_by_id(&user_id, &repost_id).await.unwrap();
    assert!(
        post_counts_result.is_none(),
        "Repost counts should not be found after deletion"
    );

    // Attempt to get post view; should not exist
    let post_view = PostView::get_by_id(&user_id, &repost_id, None, None, None)
        .await
        .unwrap();
    assert!(
        post_view.is_none(),
        "Repost view should not be found after deletion"
    );
    Ok(())
}
