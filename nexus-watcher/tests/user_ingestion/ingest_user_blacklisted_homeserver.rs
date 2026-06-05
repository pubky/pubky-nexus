use super::utils::{assert_user_ingested, create_external_test_homeserver};
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::models::error::ModelError;
use nexus_common::models::user::{UserDetails, UserIngestor};
use pubky::Keypair;
use pubky_app_specs::PubkyId;

/// A [`UserIngestor`] whose blacklist contains the user's HS refuses to ingest
/// that user, returning [`ModelError::HomeserverBlacklisted`] and leaving no
/// graph node behind.
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
        matches!(err, ModelError::HomeserverBlacklisted { .. }),
        "expected HomeserverBlacklisted, got {err:?}"
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
