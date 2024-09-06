use anyhow::Result;
use log::info;
use pkarr::{mainline::Testnet, Keypair, PublicKey};
use pubky::PubkyClient;
use pubky_nexus::{
    models::pubky_app::{PubkyAppUser, UserLink},
    setup, Config,
};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env();
    setup(&config).await;

    // Initialize the PubkyClient based on configuration
    let client = match config.testnet {
        true => {
            let testnet = Testnet {
                bootstrap: vec![config.bootstrap.clone()],
                nodes: vec![],
            };
            PubkyClient::builder().testnet(&testnet).build()
        }
        false => PubkyClient::default(),
    };

    // Generate a random keypair
    let keypair = Keypair::random();
    let pk = keypair.public_key().to_z32();
    info!("The pubky id is: {}", pk);

    // Convert the homeserver from the config into a PublicKey
    let homeserver = PublicKey::try_from(config.homeserver.as_str())?;

    // Perform signup
    client.signup(&keypair, &homeserver).await?;

    // Create a new profile
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

    // Serialize the profile to JSON
    let profile_json = serde_json::to_vec(&user)?;

    // Put some content into the Pubky system
    let url = format!("pubky://{}/pub/pubky-app/profile.json", pk);
    client.put(url.as_str(), &profile_json).await?;

    Ok(())
}
