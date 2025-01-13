use criterion::{criterion_group, criterion_main, Criterion};
use pkarr::mainline::Testnet;
use pubky::PubkyClient;
use pubky_app_specs::{PubkyAppUser, PubkyAppUserLink};
use pubky_common::crypto::Keypair;
use pubky_homeserver::Homeserver;
use pubky_nexus::{
    events::retry::{RetryManager, SenderChannel},
    types::PubkyId,
    EventProcessor,
};
use setup::run_setup;
use std::time::Duration;
use tokio::{runtime::Runtime, sync::mpsc};

mod setup;

/// Creates a homeserver and:
/// 1. Created a user
/// 2. Sign up the user
/// 3. Upload a profile.json
/// 4. Delete the profile.json
async fn create_homeserver_with_events() -> (Testnet, String, SenderChannel) {
    // Create the test environment
    let testnet = Testnet::new(3);
    let homeserver = Homeserver::start_test(&testnet).await.unwrap();
    let client = PubkyClient::test(&testnet);
    let homeserver_url = format!("http://localhost:{}", homeserver.port());

    // Generate user data
    let keypair = Keypair::random();
    let user_id = keypair.public_key().to_z32();

    let retry_manager = RetryManager::initialise(mpsc::channel(1024));
    // Prepare the sender channel to send the messages to the retry manager
    let sender_clone = retry_manager.sender.clone();
    // Create new asynchronous task to control the failed events
    tokio::spawn(async move {
        let _ = retry_manager.exec().await;
    });

    // Create and delete a user profile (as per your requirement)
    client
        .signup(&keypair, &homeserver.public_key())
        .await
        .unwrap();

    let user = PubkyAppUser {
        bio: Some("This is an example bio".to_string()),
        image: Some("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAAXNSR0IArs4c6QAAAA1JREFUGFdjiO4O+w8ABL0CPPcYQa4AAAAASUVORK5CYII=".to_string()),
        links: Some(vec![PubkyAppUserLink {
            title: "My Website".to_string(),
            url: "https://example.com".to_string(),
        }]),
        name: "Benchmark User".to_string(),
        status: Some("Running Tests".to_string()),
    };

    let profile_json = serde_json::to_vec(&user).unwrap();
    let url = format!("pubky://{}/pub/pubky.app/profile.json", user_id);

    // Create user profile
    client.put(url.as_str(), &profile_json).await.unwrap();

    // Delete the user profile
    client.delete(url.as_str()).await.unwrap();

    (testnet, homeserver_url, sender_clone)
}

fn bench_create_delete_user(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking write and delete user on homeserver");
    println!("***************************************");

    run_setup();

    // Set up the environment only once
    let rt = Runtime::new().unwrap();
    let (_, homeserver_url, sender) = rt.block_on(create_homeserver_with_events());

    c.bench_function("create_delete_homeserver_user", |b| {
        b.to_async(&rt).iter(|| {
            let sender_clone = sender.clone(); // Clone the sender for each iteration
            let homeserver_url_clone = homeserver_url.clone();
            async move {
                // Create hardcoded homeserver pubkyId
                let id = PubkyId::try_from("66h9hkdaud4ekkuummh3b4zhk68iggzirqbomyktfhq5s84jirno")
                    .unwrap();

                // Benchmark the event processor initialization and run
                let mut event_processor =
                    EventProcessor::test(homeserver_url_clone, id, sender_clone).await;
                event_processor.run().await.unwrap();
            }
        });
    });
}

fn configure_criterion() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::new(10, 0))
        .sample_size(20)
        .warm_up_time(Duration::new(3, 0))
}

criterion_group! {
    name = benches;
    config = configure_criterion();
    targets = bench_create_delete_user,
}

criterion_main!(benches);
