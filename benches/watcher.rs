use criterion::{criterion_group, criterion_main, Criterion};
use pkarr::{mainline::Testnet, Keypair};
use pubky::PubkyClient;
use pubky_homeserver::Homeserver;
use pubky_nexus::models::pubky_app::{PubkyAppUser, UserLink};
use pubky_nexus::EventProcessor;
use setup::run_setup;
use std::time::Duration;
use tokio::runtime::Runtime;

mod setup;

/// Creates a homeserver and:
/// 1. Created a user
/// 2. Sign up the user
/// 3. Upload a profile.json
/// 4. Delete the profile.json
async fn create_homeserver_with_events() -> (Testnet, String) {
    // Create the test environment
    let testnet = Testnet::new(3);
    let homeserver = Homeserver::start_test(&testnet).await.unwrap();
    let client = PubkyClient::test(&testnet);
    let homeserver_url = format!("http://localhost:{}", homeserver.port());

    // Generate user data
    let keypair = Keypair::random();
    let user_id = keypair.public_key().to_z32();

    // Create and delete a user profile (as per your requirement)
    client
        .signup(&keypair, &homeserver.public_key())
        .await
        .unwrap();

    let user = PubkyAppUser {
        bio: Some("This is an example bio".to_string()),
        image: Some("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAAXNSR0IArs4c6QAAAA1JREFUGFdjiO4O+w8ABL0CPPcYQa4AAAAASUVORK5CYII=".to_string()),
        links: Some(vec![UserLink {
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

    (testnet, homeserver_url)
}

fn bench_create_delete_user(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking write and delete user on homeserver");
    println!("***************************************");

    run_setup();

    // Set up the environment only once
    let rt = Runtime::new().unwrap();
    let (testnet, homeserver_url) = rt.block_on(create_homeserver_with_events());

    c.bench_function("create_delete_homeserver_user", |b| {
        b.to_async(&rt).iter(|| async {
            // Benchmark the event processor initialization and run
            let mut event_processor = EventProcessor::test(&testnet, homeserver_url.clone()).await;
            event_processor.run().await.unwrap();
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
