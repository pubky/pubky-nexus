use log::info;
use pkarr::{mainline::Testnet, Keypair, PublicKey};
use pubky::PubkyClient;
use pubky_nexus::{setup, Config};
use reqwest::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env();
    setup(&config).await;

    // Initialize the PubkyClient based on configuration
    let client = match config.testnet {
        true => {
            let testnet = Testnet {
                bootstrap: vec![config.bootstrap.clone()],
                nodes: vec![],
            };
            PubkyClient::test(&testnet)
        }
        false => PubkyClient::new(),
    };

    // Generate a random keypair
    let keypair = Keypair::random();
    let pk = keypair.public_key().to_z32();
    info!("The pubky id is: {}", pk);

    // Convert the homeserver from the config into a PublicKey
    let homeserver = PublicKey::try_from(config.homeserver.as_str())?;

    // Perform signup
    client.signup(&keypair, &homeserver).await?;

    // Put some content into the Pubky system
    let url = format!("pubky://{}/pub/pubky-app/post/0000", pk);
    let content = vec![0u8; 5]; // Create a small content
    client.put(url.as_str(), &content).await?;

    // List the content at a specific URL
    let list = client.list(format!("pubky://{}/pub/pubky-app/", pk).as_str())?;
    let result = list.send().await?;
    info!("Listed content: {:?}", result);

    // Create an HTTP client using `reqwest`
    let http_client = Client::new();
    let res = http_client
        .get(format!("{}/events/", config.homeserver_url))
        .send()
        .await?
        .text()
        .await?;

    // Split the response into lines
    let lines: Vec<&str> = res.trim().split('\n').collect();

    // Pretty print each line
    for line in &lines {
        if line.starts_with("PUT") || line.starts_with("DEL") {
            // Parse the event type and URL
            let parts: Vec<&str> = line.splitn(2, ' ').collect();
            if parts.len() == 2 {
                let event_type = parts[0];
                let url = parts[1];
                info!("Event: {} | URL: {}", event_type, url);
            }
        } else if line.starts_with("cursor:") {
            if let Some(cursor) = line.strip_prefix("cursor: ") {
                info!("Cursor for the next request: {}", cursor);
            }
        }
    }

    Ok(())
}
