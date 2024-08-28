use anyhow::Result;
use pkarr::{mainline::Testnet, Keypair};
use pubky::PubkyClient;
use pubky_homeserver::Homeserver;
use pubky_nexus::{
    models::{
        post::PostView,
        pubky_app::{traits::GenerateId, PostKind, PubkyAppPost, PubkyAppUser},
    },
    setup, Config, EventProcessor,
};

#[tokio::test]
async fn test_homeserver_post() -> Result<()> {
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

    let user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Test Poster".to_string(),
        status: None,
    };

    // Serialize the profile to JSON
    let profile_json = serde_json::to_vec(&user)?;

    // Put user profile into the homeserver
    let url = format!("pubky://{}/pub/pubky-app/profile.json", user_id);
    client.put(url.as_str(), &profile_json).await?;

    // Index to Nexus from Homeserver using the events processor
    event_processor.run().await.unwrap();

    // Create a post sticking to the homeserver schema for pubky-app profiles
    let post = PubkyAppPost {
        content: "This is a test post!".to_string(),
        kind: PostKind::Short,
        embed: None,
    };

    let post_id = post.create_id();

    // Serialize the post to JSON
    let post_json = serde_json::to_vec(&post)?;

    // Put post into the homeserver
    let url = format!("pubky://{}/pub/pubky-app/posts/{}", user_id, post_id);
    client.put(url.as_str(), &post_json).await?;

    // Index to Nexus from Homeserver using the events processor
    event_processor.run().await.unwrap();

    // Assert the new post can be served from Nexus
    let result_post = PostView::get_by_id(&user_id, &post_id, None, None, None)
        .await
        .unwrap()
        .expect("The new post was not served from Nexus");

    // Clean UP
    // Delete the user from the homeserver
    client.delete(url.as_str()).await?;

    // Clean up nexus indexing the new delete event
    event_processor.run().await.unwrap();

    Ok(())
}
