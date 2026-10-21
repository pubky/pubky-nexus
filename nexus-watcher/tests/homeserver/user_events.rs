//! The `/events-stream` request the client builds: which homeserver and user it asks, from
//! which cursor, and that a rate-limited request is retried. The count is capped by the
//! client itself (`read_user_events`), so it is covered by a unit test instead, and the
//! `/pub/` path filter cannot be observed, because the homeserver accepts writes only under
//! `/pub/`.

use std::time::{Duration, Instant};

use crate::event_processor::utils::watcher::WatcherTest;
use nexus_common::types::DynError;
use pubky::{Keypair, PublicKey};
use pubky_app_specs::{PubkyAppUser, PubkyId};
use pubky_testnet::pubky_homeserver::quota_config::{
    GlobPattern, HttpMethod, LimitKeyType, PathLimit,
};
use pubky_testnet::pubky_homeserver::{ConfigToml, ConnectionString, MockDataDir};

fn profile(name: &str) -> PubkyAppUser {
    PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: name.to_string(),
        status: None,
    }
}

/// A user's events come from the homeserver they are asked of, not the one their PKARR
/// record names, and a later request from the last cursor returns only the events after it.
#[tokio_shared_rt::test(shared)]
async fn test_user_events_come_from_the_homeserver_after_the_cursor() -> Result<(), DynError> {
    let mut test = WatcherTest::setup(None).await?;
    let user_kp = Keypair::random();
    let user_id = test.create_user(&user_kp, &profile("first")).await?;
    test.create_profile(&user_kp, &profile("second")).await?;
    // The user's PKARR record now names another homeserver, so only a request to the one
    // asked finds these events.
    let other_hs = test.testnet.create_random_homeserver().await?.public_key();
    test.register_user_in_hs(&user_kp, &other_hs).await?;
    let client = &test.event_processor_runner.client;
    let user_pk = user_kp.public_key();

    let first = client
        .user_events(&test.homeserver_id, &user_pk, 0, 1)
        .await?;

    assert_eq!(first.len(), 1, "{first:?}");
    assert_eq!(first[0].owner().z32(), user_id);

    let cursor = first[0].cursor;
    let rest = client
        .user_events(&test.homeserver_id, &user_pk, cursor, 100)
        .await?;

    assert_eq!(rest.len(), 1, "{rest:?}");
    assert!(rest[0].cursor > cursor, "{rest:?}");
    assert_eq!(rest[0].owner().z32(), user_id);

    Ok(())
}

/// A homeserver in `test`'s network that answers one `/events-stream` request a second per
/// client address, and 429 to the rest.
async fn homeserver_allowing_one_stream_request_a_second(
    test: &mut WatcherTest,
) -> Result<PublicKey, DynError> {
    let mut config = ConfigToml::default_test_config();
    // The stack's test database, as `Testnet::create_random_homeserver` reads it.
    if let Ok(connection_string) = std::env::var("TEST_PUBKY_CONNECTION_STRING") {
        config.general.database_url = ConnectionString::new(&connection_string)?;
    }
    config.drive.rate_limits.push(PathLimit {
        path: GlobPattern::new("/events-stream"),
        method: HttpMethod::from(http::Method::GET),
        quota: "1r/s".parse()?,
        key: LimitKeyType::Ip,
        burst: None,
        whitelist: vec![],
    });
    let mock_dir = MockDataDir::new(config, Some(Keypair::random()))?;
    let homeserver = test
        .testnet
        .create_homeserver_app_with_mock(mock_dir)
        .await?;
    Ok(homeserver.public_key())
}

/// A rate-limited `/events-stream` request is retried through the real client: a homeserver
/// that allows one such request a second still answers the second request in a row, once
/// the client has waited out the 429.
#[tokio_shared_rt::test(shared)]
async fn test_rate_limited_user_events_are_retried_against_a_homeserver() -> Result<(), DynError> {
    let mut test = WatcherTest::setup(None).await?;
    let hs = homeserver_allowing_one_stream_request_a_second(&mut test).await?;
    let user_kp = Keypair::random();
    test.register_user_in_hs(&user_kp, &hs).await?;
    let client = &test.event_processor_runner.client;
    let hs_id = PubkyId::from(hs);
    let user_pk = user_kp.public_key();

    client.user_events(&hs_id, &user_pk, 0, 10).await?;
    let asked_again = Instant::now();
    let events = client.user_events(&hs_id, &user_pk, 0, 10).await?;

    assert!(events.is_empty(), "{events:?}");
    assert!(
        asked_again.elapsed() >= Duration::from_secs(1),
        "the second request should have waited out a 429, took {:?}",
        asked_again.elapsed()
    );

    Ok(())
}
