use crate::watcher::{users::utils::find_user_details, utils::WatcherTest};
use anyhow::Result;
use pubky_common::crypto::Keypair;
use pubky_nexus::models::{
    pubky_app::PubkyAppUser,
    user::{UserCounts, UserView},
};

#[tokio::test]
async fn test_delete_user_with_relationships() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create a new user without any relationships
    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("Test user without relationships".to_string()),
        image: None,
        links: None,
        name: "Watcher:UserDel:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    // Delete the user
    test.cleanup_user(&user_id).await?;

    // Attempt to find user details; should not exist
    let user_details_result = find_user_details(&user_id).await;
    assert!(
        user_details_result.is_err(),
        "User details should not be found after deletion"
    );

    // Attempt to find user counts; should not exist
    let user_counts_result = UserCounts::get_by_id(&user_id).await.unwrap();
    assert!(
        user_counts_result.is_none(),
        "User counts should not be found after deletion"
    );

    // Attempt to get user view; should not exist
    let user_view = UserView::get_by_id(&user_id, None).await.unwrap();
    assert!(
        user_view.is_none(),
        "User view should not be found after deletion"
    );

    Ok(())
}
