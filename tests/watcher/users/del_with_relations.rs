use crate::watcher::{
    users::utils::{find_user_counts, find_user_details},
    utils::WatcherTest,
};
use anyhow::Result;
use pubky_common::crypto::Keypair;
use pubky_nexus::models::{
    pubky_app::{PostKind, PubkyAppPost, PubkyAppUser},
    user::{UserCounts, UserView},
};

#[tokio::test]
async fn test_delete_user_with_relationships() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create a new user
    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("Test user with relationships".to_string()),
        image: None,
        links: None,
        name: "UserWithRelationships".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    // Create a post to establish a relationship
    let post = PubkyAppPost {
        content: "User's first post".to_string(),
        kind: PostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };
    let post_id = test.create_post(&user_id, &post).await?;

    // Delete the user
    test.cleanup_user(&user_id).await?;

    // Fetch user details; should be updated to "[DELETED]"
    let user_details = find_user_details(&user_id).await?;
    assert_eq!(
        user_details.name, "[DELETED]",
        "User name should be '[DELETED]' after deletion"
    );
    assert_eq!(
        user_details.bio,
        Some("null".to_string()),
        "User bio should be 'null' after deletion.",
    );
    assert_eq!(
        user_details.status,
        Some("null".to_string()),
        "User status should be None after deletion"
    );
    assert_eq!(
        user_details.image,
        Some("null".to_string()),
        "User image should be None after deletion"
    );

    // User counts should still exist
    let user_counts = find_user_counts(&user_id).await;
    assert_eq!(
        user_counts.posts, 1,
        "User should still have posts count after deletion"
    );

    // User view should reflect the updated details
    let user_view = UserView::get_by_id(&user_id, None).await.unwrap();
    assert!(
        user_view.is_some(),
        "User view should be present after deletion"
    );
    let user_view = user_view.unwrap();
    assert_eq!(
        user_view.details.name, "[DELETED]",
        "User view name should be '[DELETED]' after deletion"
    );

    // Now delete the user's post
    test.cleanup_post(&user_id, &post_id).await?;

    // Write and delete the user again; this time it should be fully removed
    test.create_user(&keypair, &user).await?;
    test.cleanup_user(&user_id).await?;

    // Attempt to find user details; should not exist
    let user_details_result = find_user_details(&user_id).await;
    assert!(
        user_details_result.is_err(),
        "User details should not be found after final deletion"
    );

    // Attempt to find user counts; should not exist
    let user_counts_result = UserCounts::get_by_id(&user_id).await.unwrap();
    assert!(
        user_counts_result.is_none(),
        "User counts should not be found after deletion"
    );

    // User view should not be found
    let user_view = UserView::get_by_id(&user_id, None).await.unwrap();
    assert!(
        user_view.is_none(),
        "User view should not be found after final deletion"
    );

    Ok(())
}
