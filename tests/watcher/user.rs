use anyhow::Result;
use log::info;
use pkarr::{mainline::Testnet, Keypair};
use pubky::PubkyClient;
use pubky_homeserver::Homeserver;
use pubky_nexus::{
    models::user::{HomeserverUser, UserLink, UserView},
    setup, Config, EventProcessor,
};

#[tokio::test]
async fn test_homeserver_user() -> Result<()> {
    let config = Config::from_env();
    setup(&config).await;

    let testnet = Testnet::new(10);
    let homeserver = Homeserver::start_test(&testnet).await.unwrap();

    let client = PubkyClient::test(&testnet);

    let homeserver_url = format!("http://localhost:{}", homeserver.port());
    let mut event_processor = EventProcessor::test(&testnet, homeserver_url).await;

    // Write new user profile to homeserver
    let keypair = Keypair::random();
    let user_id = keypair.public_key().to_z32();

    client
        .signup(&keypair, &homeserver.public_key())
        .await
        .unwrap();

    // Create a user sticking to the homeserver schema for pubky-app profiles
    let user = HomeserverUser {
        bio: Some("This is an example bio".to_string()),
        image: Some("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAAXNSR0IArs4c6QAAAA1JREFUGFdjiO4O+w8ABL0CPPcYQa4AAAAASUVORK5CYII=".to_string()),
        links: Some(vec![UserLink {
            title: "My Website".to_string(),
            url: "https://example.com".to_string(),
        }]),
        name: "Satoshi Nakamoto".to_string(),
        status: Some("Running Bitcoin".to_string()),
    };

    // Serialize the profile to JSON
    let profile_json = serde_json::to_vec(&user)?;

    // Put user profile into the homeserver
    let url = format!("pubky://{}/pub/pubky-app/profile.json", user_id);
    client.put(url.as_str(), &profile_json).await?;

    // Index to Nexus from Homeserver using the events processor
    event_processor.run().await.unwrap();

    // Assert the new user can be served from Nexus
    let result_user = UserView::get_by_id(&user_id, None)
        .await
        .unwrap()
        .expect("The new user was not served from Nexus");

    info!("New user served: {:?}", result_user);
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

    // Delete the user from the homeserver
    client.delete(url.as_str()).await?;

    // Index the new delete event to Nexus
    event_processor.run().await.unwrap();

    // Assert the new user does not exist in Nexus
    let result = UserView::get_by_id(&user_id, None).await.unwrap();

    assert!(result.is_none(), "The user should have been deleted");

    Ok(())
}
