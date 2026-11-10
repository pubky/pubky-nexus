use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::utils::test_utils::default_ingestor_tests;
use nexus_watcher::events::handlers;
use nexus_watcher::EventProcessorError;
use pubky::Keypair;
use pubky_social_specs::legacy_v0::PubkyAppUser;
use pubky_social_specs::PubkyId;

/// A canonical z-base32 id whose bytes are no Ed25519 curve point: it parses as an id, but no
/// homeserver can ever be looked up for it.
const NOT_A_PUBLIC_KEY: &str = "yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyo";

/// A follow of an unknown user whose id is no public key must fail with the non-retryable
/// [`EventProcessorError::SpecValidation`] instead of `MissingDependency`: the followee can never
/// be ingested, so retrying the follow only fills the retry queue.
#[tokio_shared_rt::test(shared)]
async fn test_follow_of_an_id_that_is_no_public_key_is_dropped() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let followee_id = PubkyId::try_from(NOT_A_PUBLIC_KEY).expect("a canonical id");
    assert!(
        followee_id.to_public_key().is_err(),
        "the fixture must not be a curve point"
    );

    let follower_kp = Keypair::random();
    let follower = PubkyAppUser {
        bio: Some("test_follow_of_an_id_that_is_no_public_key_is_dropped".to_string()),
        image: None,
        links: None,
        name: "Watcher:UserIngestion:FollowInvalidKey".to_string(),
        status: None,
    };
    test.create_user(&follower_kp, &follower).await?;

    let err = handlers::follow::sync_put(
        PubkyId::from(follower_kp.public_key()),
        followee_id,
        &default_ingestor_tests(),
    )
    .await
    .expect_err("a follow of an id that is no public key must fail");

    assert!(
        matches!(err, EventProcessorError::SpecValidation(_)),
        "expected SpecValidation, got {err:?}"
    );

    test.cleanup_user(&follower_kp).await?;
    Ok(())
}
