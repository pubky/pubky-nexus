use anyhow::Result;
use log::info;
use mainline::Testnet;
use pubky::Client;
use pubky_app_specs::{traits::HasPath, PubkyAppUser, PubkyAppUserLink, PROTOCOL};
use pubky_common::crypto::{Keypair, PublicKey};
use pubky_nexus::{setup, Config};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env();
    setup(&config).await;

    // Initialize the Client based on configuration
    let client = match config.testnet {
        true => {
            let testnet = Testnet {
                bootstrap: vec![config.bootstrap.clone()],
                nodes: vec![],
            };
            Client::builder().testnet(&testnet).build()?
        }
        false => Client::new()?,
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
    let user = PubkyAppUser::new(
        "Satoshi Nakamoto".to_string(),
        Some("This is an example bio".to_string()),
        Some("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAAXNSR0IArs4c6QAAAA1JREFUGFdjiO4O+w8ABL0CPPcYQa4AAAAASUVORK5CYII=".to_string()),
        Some(vec![PubkyAppUserLink {
            title: "My Website".to_string(),
            url: "https://example.com".to_string(),
        }]),
        Some("Running Bitcoin".to_string()),
    );

    // Serialize the profile to JSON
    let profile_json = serde_json::to_vec(&user)?;

    // Put some content into the Pubky homeserver
    let url = format!(
        "{protocol}{pk}{path}",
        protocol = PROTOCOL,
        pk = pk,
        path = user.create_path()
    );
    client.put(url.as_str()).json(&profile_json).send().await?;

    Ok(())
}
