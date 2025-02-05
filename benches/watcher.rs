use criterion::{criterion_group, criterion_main, Criterion};
use mainline::Testnet;
use pubky::Client;
use pubky_app_specs::{PubkyAppUser, PubkyAppUserLink, PubkyId};
use pubky_common::crypto::Keypair;
use pubky_homeserver::Homeserver as PubkyCoreHomeserver;
use pubky_nexus::{models::homeserver::Homeserver, EventProcessor};
use setup::run_setup;
use std::time::Duration;
use tokio::runtime::Runtime;

mod setup;

/// Create a homeserver and:
/// 1. Create a user
/// 2. Sign up the user
/// 3. Upload a profile.json
/// 4. Delete the profile.json
async fn create_homeserver_with_events() -> (Testnet, String) {
    // Create the test environment
    let testnet = Testnet::new(3).unwrap();
    let homeserver = PubkyCoreHomeserver::start_test(&testnet).await.unwrap();
    let client = Client::builder().testnet(&testnet).build().unwrap();
    let homeserver_id = PubkyId::try_from(homeserver.public_key().to_string().as_str()).unwrap();

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
    client
        .put(url.as_str())
        .json(&profile_json)
        .send()
        .await
        .unwrap();

    // Delete the user profile
    client.delete(url.as_str()).send().await.unwrap();

    (testnet, homeserver_url)
}

fn bench_create_delete_user(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking write and delete user on homeserver");
    println!("***************************************");

    run_setup();

    // Set up the environment only once
    let rt = Runtime::new().unwrap();
    let (_, homeserver_id) = rt.block_on(create_homeserver_with_events());

    c.bench_function("create_delete_homeserver_user", |b| {
        b.to_async(&rt).iter(|| {
            let homeserver_id_clone = homeserver_id.clone(); // Clone the sender for each iteration
            async move {
                // Benchmark the event processor initialization and run
                let mut event_processor =
                    EventProcessor::test(homeserver_id_clone.to_string()).await;

                let homeserver = Homeserver::new(homeserver_id_clone);
                event_processor.run(homeserver).await.unwrap();
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
