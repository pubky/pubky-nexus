use log::info;
use pkarr::{mainline::Testnet, Keypair, PublicKey};
use pubky::PubkyClient;
use pubky_nexus::{models::user::UserLink, setup, Config};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct Profile {
    bio: Option<String>,
    image: Option<String>,
    links: Option<Vec<UserLink>>,
    name: String,
}

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

    // Create a new profile
    let profile = Profile {
        bio: Some("This is my bio".to_string()),
        image: Some("base64_image_string_or_pubky_uri".to_string()),
        links: Some(vec![UserLink {
            title: "My Website".to_string(),
            url: "https://example.com".to_string(),
        }]),
        name: "John Doe".to_string(),
    };

    // Serialize the profile to JSON
    let profile_json = serde_json::to_vec(&profile)?;

    // Put some content into the Pubky system
    let url = format!("pubky://{}/pub/pubky-app/profile.json", pk);
    client.put(url.as_str(), &profile_json).await?;

    // List the content at a specific URL
    let list = client
        .list(format!("pubky://{}/pub/pubky-app/", pk).as_str())?
        .cursor("0")
        .limit(10_000);
    let result = list.send().await?;
    info!("Listed content: {:?}", result);

    // Create an HTTP client using `reqwest`
    let http_client = Client::new();
    let cursor = "0";
    let limit = 10_000;
    let res = http_client
        .get(format!(
            "{}/events/?cursor={}&limit={}",
            config.homeserver_url, cursor, limit
        ))
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

                if url.ends_with("profile.json") {
                    // Retrieve the profile using `client.get()` and print it to the console
                    let retrieved_profile = client.get(url).await?.unwrap_or_default();

                    // Deserialize the retrieved profile JSON back into the Profile struct
                    let retrieved_profile: Profile = serde_json::from_slice(&retrieved_profile)?;

                    // Print the retrieved profile
                    println!("Retrieved Profile: {:?}", retrieved_profile);
                }
            }
        } else if line.starts_with("cursor:") {
            if let Some(cursor) = line.strip_prefix("cursor: ") {
                info!("Cursor for the next request: {}", cursor);
            }
        }
    }

    Ok(())
}
