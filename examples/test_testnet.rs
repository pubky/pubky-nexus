use pubky::Keypair;
use pubky_testnet::Testnet;

#[tokio::main]
async fn main() {
    // Run a new testnet.
    let testnet = Testnet::run().await.unwrap();

    // Optionally create and run a Homeserver.
    let server = testnet.run_homeserver().await.unwrap();

    // Create a Pubky Client from the testnet.
    let client = testnet.client_builder().build().unwrap();

    let keypair = Keypair::random();

    client.signup(&keypair, &server.public_key()).await.unwrap();
    client
        .put(format!(
            "pubky://{}/pub/pubky.app/object",
            keypair.public_key(),
        ))
        .body([0, 1, 2, 3].to_vec())
        .send()
        .await
        .unwrap();

    let response = client
        .get(format!(
            "https://{}/events/?cursor=0000000000000&limit=1000",
            server.public_key()
        ))
        .send()
        .await
        .unwrap();

    println!("Event Lines: {}", response.text().await.unwrap())
}
