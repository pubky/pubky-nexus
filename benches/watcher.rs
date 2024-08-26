use criterion::{criterion_group, criterion_main, Criterion};
use pkarr::{mainline::Testnet, Keypair};
use pubky::PubkyClient;
use pubky_homeserver::Homeserver;
use pubky_nexus::models::homeserver::{HomeserverUser, UserLink};
use pubky_nexus::models::user::UserView;
use pubky_nexus::EventProcessor;
use setup::run_setup;
use std::time::Duration;
use tokio::runtime::Runtime;

mod setup;

fn bench_create_delete_user(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking write and delete user on homeserver");
    println!("***************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function(
        "create_delete_user",
        |b| {
            b.to_async(&rt).iter(|| async {
                // TODO: extract set up the environment for the test. Run once.
                let testnet = Testnet::new(3);
                let homeserver = Homeserver::start_test(&testnet).await.unwrap();
                let client = PubkyClient::test(&testnet);
                let homeserver_url = format!("http://localhost:{}", homeserver.port());
                let mut event_processor = EventProcessor::test(&testnet, homeserver_url).await;

                // Generate user data
                let keypair = Keypair::random();
                let user_id = keypair.public_key().to_z32();

                client
                    .signup(&keypair, &homeserver.public_key())
                    .await
                    .unwrap();

                let user = HomeserverUser {
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
                let url = format!("pubky://{}/pub/pubky-app/profile.json", user_id);

                // Create user profile
                client.put(url.as_str(), &profile_json).await.unwrap();
                event_processor.run().await.unwrap();

                // Verify user creation
                let result_user = UserView::get_by_id(&user_id, None)
                    .await
                    .unwrap()
                    .expect("The new user was not served from Nexus");
                criterion::black_box(result_user);

                // Delete the user profile
                client.delete(url.as_str()).await.unwrap();
                event_processor.run().await.unwrap();

                // Verify user deletion
                let result = UserView::get_by_id(&user_id, None).await.unwrap();
                assert!(result.is_none(), "The user should have been deleted");
            });
        },
    );
}

fn configure_criterion() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::new(5, 0))
        .sample_size(20)
        .warm_up_time(Duration::new(1, 0))
}

criterion_group! {
    name = benches;
    config = configure_criterion();
    targets = bench_create_delete_user,
}

criterion_main!(benches);
