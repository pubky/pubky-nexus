use anyhow::Result;
use pkarr::{mainline::Testnet, Keypair, PublicKey};
use pubky::PubkyClient;

const HOMESERVER_URL: &str = "http://localhost:15411";

#[tokio::test]
async fn test_pubky_client() -> Result<()> {
    let testnet = Testnet {
        bootstrap: vec!["127.0.0.1:6881".to_string()],
        nodes: vec![],
    };
    let client = PubkyClient::test(&testnet);

    // let client = PubkyClient::new();

    let keypair = Keypair::random();

    let pk = keypair.public_key().to_z32();
    println!("The pubky id is: {}", pk);

    let homeserver = PublicKey::try_from("8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo")?;

    client.signup(&keypair, &homeserver).await?;

    let url = format!("pubky://{}/pub/pubky-app/post/0000", pk);
    let content = vec![0u8; 5];

    client.put(url.as_str(), &content).await?;

    let list = client.list(format!("pubky://{}/pub/pubky-app/", pk).as_str())?;
    let result = list.send().await?;

    println!("{:?}", result);

    let http_client = httpc_test::new_client(HOMESERVER_URL)?;

    let res = http_client.do_get("/events/").await?;

    // Assuming there's a `text()` method to get the response body as a string
    let body_str = res.text_body()?;

    // Split the response into lines
    let lines: Vec<&str> = body_str.trim().split('\n').collect();

    // Pretty print each line
    for line in &lines {
        if line.starts_with("PUT") || line.starts_with("DEL") {
            // Parse the event type and URL
            let parts: Vec<&str> = line.splitn(2, ' ').collect();
            if parts.len() == 2 {
                let event_type = parts[0];
                let url = parts[1];
                println!("Event: {} | URL: {}", event_type, url);
            }
        } else {
            // Print the cursor (last line)
            println!("Cursor for the next request: {}", line);
        }
    }

    Ok(())
}
