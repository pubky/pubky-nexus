//! The `/events-stream` request the client builds: which homeserver and user it asks, from
//! which cursor. The count is capped by the client itself (`read_user_events`), so it is
//! covered by a unit test instead, and the `/pub/` path filter cannot be observed, because
//! the homeserver accepts writes only under `/pub/`.

use crate::event_processor::utils::watcher::WatcherTest;
use nexus_common::types::DynError;
use pubky::Keypair;
use pubky_app_specs::PubkyAppUser;

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
