use crate::watcher::{
    users::utils::{find_user_counts, find_user_details},
    utils::watcher::WatcherTest,
};
use anyhow::Result;
use chrono::Utc;
use pkarr::Keypair;
use pubky_app_specs::{
    traits::HasPath, PubkyAppBlob, PubkyAppFile, PubkyAppPost, PubkyAppPostKind, PubkyAppUser,
    PubkyAppUserLink,
};
use pubky_nexus::{
    models::user::{UserCounts, UserView},
    PubkyConnector,
};
use serde_json::to_vec;
#[tokio_shared_rt::test(shared)]
async fn test_delete_user_with_relationships() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create a new user
    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_delete_user_with_relationships".to_string()),
        image: None,
        links: None,
        name: "Watcher:UserDeleteWith:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    // Create a post to establish a relationship
    let post = PubkyAppPost {
        content: "User's first post".to_string(),
        kind: PubkyAppPostKind::Short,
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
        user_details.bio, None,
        "User bio should be 'null' after deletion.",
    );
    assert_eq!(
        user_details.status, None,
        "User status should be None after deletion"
    );
    assert_eq!(
        user_details.image, None,
        "User image should be None after deletion"
    );

    // User counts should still exist
    let user_counts = find_user_counts(&user_id).await;
    assert_eq!(
        user_counts.posts, 1,
        "User should still have posts count after deletion"
    );

    // User view should reflect the updated details
    let user_view = UserView::get_by_id(&user_id, None, None).await.unwrap();
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
    let user_view = UserView::get_by_id(&user_id, None, None).await.unwrap();
    assert!(
        user_view.is_none(),
        "User view should not be found after final deletion"
    );

    // Create a User with image and links
    let keypair = Keypair::random();
    let mut user_with = PubkyAppUser {
        bio: Some("test_delete_user_with_relationships".to_string()),
        image: None,
        links: None,
        name: "Watcher:UserDeleteWith:UserWith".to_string(),
        status: None,
    };
    let user_with_id = test.create_user(&keypair, &user_with).await?;

    // Add image to the user
    let blob_data = "Image bytes blob".to_string();
    let blob = PubkyAppBlob::new(blob_data.as_bytes().to_vec());
    let blob_url = format!("pubky://{}{}", user_with_id, blob.create_path());

    let pubky_client = PubkyConnector::get_pubky_client()?;
    pubky_client
        .put(blob_url.as_str())
        .body(blob.0.clone())
        .send()
        .await?;

    // Act
    let file = PubkyAppFile {
        name: "Watcher:UserDeleteWith:UserWith:Image".to_string(),
        content_type: "image/png".to_string(),
        src: blob_url.clone(),
        size: blob.0.len() as i64,
        created_at: Utc::now().timestamp_millis(),
    };

    let (file_id, image_url) = test.create_file(&user_with_id, &file).await?;

    user_with = PubkyAppUser {
        bio: Some("test_delete_user_with_relationships".to_string()),
        image: Some(image_url),
        links: Some(vec![PubkyAppUserLink {
            title: "Heaven".to_string(),
            url: "pubky://rest.now".to_string(),
        }]),
        name: "Watcher:UserDeleteWith:UserWith".to_string(),
        status: Some("Zombie soon".to_string()),
    };
    let _ = test.create_user(&keypair, &user_with).await?;

    // Create a post to establish a relationship
    let post_b = PubkyAppPost {
        content: "User's Second post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };
    let post_b_id = test.create_post(&user_with_id, &post_b).await?;

    // Delete the user
    test.cleanup_user(&user_with_id).await?;

    // Fetch user details; should be updated to "[DELETED]"
    let user_details = find_user_details(&user_with_id).await?;
    assert_eq!(
        user_details.name, "[DELETED]",
        "User name should be '[DELETED]' after deletion"
    );
    assert_eq!(
        user_details.bio, None,
        "User bio should be 'null' after deletion.",
    );
    assert_eq!(
        user_details.status, None,
        "User status should be None after deletion"
    );
    assert_eq!(
        user_details.image, None,
        "User image should be None after deletion"
    );

    // User counts should still exist
    let user_counts = find_user_counts(&user_with_id).await;
    assert_eq!(
        user_counts.posts, 1,
        "User should still have posts count after deletion"
    );

    // User view should reflect the updated details
    let user_view = UserView::get_by_id(&user_with_id, None, None)
        .await
        .unwrap();
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
    test.cleanup_post(&user_with_id, &post_b_id).await?;

    // Write and delete the user again; this time it should be fully removed
    test.create_user(&keypair, &user_with).await?;
    test.cleanup_user(&user_with_id).await?;
    // Delete the file
    test.cleanup_file(&user_with_id, &file_id).await?;

    // Attempt to find user details; should not exist
    let user_details_result = find_user_details(&user_with_id).await;
    assert!(
        user_details_result.is_err(),
        "User details should not be found after final deletion"
    );

    // Attempt to find user counts; should not exist
    let user_counts_result = UserCounts::get_by_id(&user_with_id).await.unwrap();
    assert!(
        user_counts_result.is_none(),
        "User counts should not be found after deletion"
    );

    // User view should not be found
    let user_view = UserView::get_by_id(&user_with_id, None, None)
        .await
        .unwrap();
    assert!(
        user_view.is_none(),
        "User view should not be found after final deletion"
    );

    Ok(())
}
