use super::utils::{assert_user_ingested, create_external_test_homeserver};
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::models::error::ModelError;
use nexus_common::models::event::EventProcessorError;
use nexus_common::models::user::{UserDetails, UserIngestor};
use nexus_watcher::events::handlers;
use pubky::Keypair;
use pubky_app_specs::{PubkyAppUser, PubkyId};

/// A [`UserIngestor`] whose blacklist contains the user's HS refuses to ingest
/// that user, returning [`ModelError::HsBlacklisted`] and leaving no graph node behind.
#[tokio_shared_rt::test(shared)]
async fn test_maybe_ingest_user_aborts_on_blacklisted_homeserver() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let hs_pk = create_external_test_homeserver(&mut test).await?;

    let user_kp = Keypair::random();
    let user_id = user_kp.public_key().to_z32();
    test.register_user_in_hs(&user_kp, &hs_pk).await?;

    let ingestor = UserIngestor::new([PubkyId::from(hs_pk.clone())]);
    let user_pubky_id = PubkyId::from(user_kp.public_key());

    let err = ingestor
        .maybe_ingest_user(&user_pubky_id)
        .await
        .expect_err("ingestion should be refused for a blacklisted HS");
    assert!(
        matches!(err, ModelError::HsBlacklisted { .. }),
        "expected HsBlacklisted, got {err:?}"
    );

    assert!(
        UserDetails::get_by_id(&user_id).await?.is_none(),
        "blacklisted user {user_id} must not be ingested"
    );

    Ok(())
}

/// Control: with an empty blacklist the same user is ingested normally, proving
/// the blacklist (not some other failure) is what blocked ingestion above.
#[tokio_shared_rt::test(shared)]
async fn test_maybe_ingest_user_ingests_when_not_blacklisted() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let hs_pk = create_external_test_homeserver(&mut test).await?;

    let user_kp = Keypair::random();
    let user_id = user_kp.public_key().to_z32();
    test.register_user_in_hs(&user_kp, &hs_pk).await?;

    let user_pubky_id = PubkyId::from(user_kp.public_key());
    UserIngestor::default()
        .maybe_ingest_user(&user_pubky_id)
        .await?;

    assert_user_ingested(&user_id, &hs_pk).await;

    Ok(())
}

/// An event depending on a user hosted by a blacklisted HS must fail with the
/// non-retryable [`EventProcessorError::HsBlacklisted`] instead of
/// `MissingDependency`, so the event is dropped instead of being retried
/// against a dependency that cannot resolve.
#[tokio_shared_rt::test(shared)]
async fn test_follow_of_user_on_blacklisted_homeserver_is_dropped() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let hs_pk = create_external_test_homeserver(&mut test).await?;

    let followee_kp = Keypair::random();
    test.register_user_in_hs(&followee_kp, &hs_pk).await?;

    let follower_kp = Keypair::random();
    let follower_user = PubkyAppUser {
        bio: Some("test_follow_of_user_on_blacklisted_homeserver".to_string()),
        image: None,
        links: None,
        name: "Watcher:UserIngestion:FollowBlacklisted".to_string(),
        status: None,
    };
    test.create_user(&follower_kp, &follower_user).await?;

    let ingestor = UserIngestor::new([PubkyId::from(hs_pk.clone())]);
    let err = handlers::follow::sync_put(
        PubkyId::from(follower_kp.public_key()),
        PubkyId::from(followee_kp.public_key()),
        &ingestor,
    )
    .await
    .expect_err("follow of a user on a blacklisted HS must fail");

    assert!(
        matches!(err, EventProcessorError::HsBlacklisted { .. }),
        "expected HsBlacklisted, got {err:?}"
    );

    Ok(())
}
