use super::utils::WatcherTest;
use anyhow::Result;
use pubky_common::crypto::Keypair;
use pubky_nexus::models::{
    pubky_app::{PubkyAppUser, UserLink},
    user::UserView,
};

#[tokio::test]
async fn test_homeserver_user() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("This is an example bio".to_string()),
        image: Some("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAAXNSR0IArs4c6QAAAA1JREFUGFdjiO4O+w8ABL0CPPcYQa4AAAAASUVORK5CYII=".to_string()),
        links: Some(vec![UserLink {
            title: "My Website".to_string(),
            url: "https://example.com".to_string(),
        }]),
        name: "Satoshi Nakamoto".to_string(),
        status: Some("Running Bitcoin".to_string()),
    };

    let user_id = test.create_user(&keypair, &user).await?;

    // Assert the new user can be served from Nexus
    let result_user = UserView::get_by_id(&user_id, None)
        .await
        .unwrap()
        .expect("The new user was not served from Nexus");

    println!("New user served: {:?}", result_user);
    assert_eq!(result_user.details.name, user.name);
    assert_eq!(result_user.details.bio, user.bio);
    assert_eq!(result_user.details.status, user.status);
    assert_eq!(result_user.counts.followers, 0);
    assert_eq!(result_user.counts.tags, 0);
    assert_eq!(result_user.counts.posts, 0);
    let result_links = result_user.details.links.unwrap_or_default();
    let expected_links = user.links.unwrap_or_default();
    for (result_link, expected_link) in result_links.iter().zip(expected_links.iter()) {
        assert_eq!(
            result_link.title, expected_link.title,
            "Link titles do not match."
        );
        assert_eq!(
            result_link.url, expected_link.url,
            "Link URLs do not match."
        );
    }

    // Cleanup
    test.cleanup_user(&user_id).await?;

    // Assert the new user does not exist in Nexus
    let result = UserView::get_by_id(&user_id, None).await.unwrap();
    assert!(result.is_none(), "The user should have been deleted");

    Ok(())
}
