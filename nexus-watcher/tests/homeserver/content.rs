//! Where the client reads a resource or a blob from: the homeserver it is asked of, whatever
//! homeserver the owner's PKARR record names, and the one the record names when it is asked
//! of none.

use super::profile;
use crate::event_processor::utils::watcher::{
    retrieve_and_handle_event_line, unrank_user, WatcherTest,
};
use nexus_common::types::DynError;
use nexus_watcher::homeserver::HsErrorKind;
use pubky::{Keypair, PubkyResource};
use pubky_social_specs::legacy_v0::PubkyAppUser;
use pubky_social_specs::PubkyId;

const PROFILE_PATH: &str = "/pub/pubky.app/profile.json";

/// The name in the profile JSON a fetch returned.
fn name_in(body: &[u8]) -> Result<String, DynError> {
    Ok(serde_json::from_slice::<PubkyAppUser>(body)?.name)
}

/// A resource and a blob are read from the homeserver asked. The user's record names another
/// homeserver, which has none of their data: asked there, both are a 404.
#[tokio_shared_rt::test(shared)]
async fn test_content_comes_from_the_homeserver_asked_not_the_one_the_record_names(
) -> Result<(), DynError> {
    let mut test = WatcherTest::setup(None).await?;
    let user_kp = Keypair::random();
    let user_id = test.create_user(&user_kp, &profile("asked")).await?;
    let other_hs = test.testnet.create_random_homeserver().await?.public_key();
    test.register_user_in_hs(&user_kp, &other_hs).await?;
    let other_hs = PubkyId::from(other_hs);
    let client = &test.event_processor_runner.client;
    let resource = PubkyResource::new(user_kp.public_key(), PROFILE_PATH)?;

    let from_first = client.get_resource(&test.homeserver_id, &resource).await;
    let from_other = client.get_resource(&other_hs, &resource).await;
    let blob_from_first = client.get_blob(Some(&test.homeserver_id), &resource).await;
    let blob_from_other = client.get_blob(Some(&other_hs), &resource).await;

    // The key now names the other homeserver, so the usual cleanup helper would delete the
    // profile there, where it never was. Undo `create_user` through the DEL handler instead.
    unrank_user(&user_id).await?;
    retrieve_and_handle_event_line(
        &format!("DEL pubky://{user_id}{PROFILE_PATH}"),
        &test.homeserver_id,
        test.event_processor_runner.event_handler.clone(),
    )
    .await?;

    assert_eq!(name_in(&from_first?)?, "asked");
    assert_eq!(name_in(&blob_from_first?)?, "asked");
    for missing in [from_other, blob_from_other] {
        let error = missing.expect_err("the other homeserver has none of the user's data");
        assert_eq!(error.hs, *other_hs, "{error:?}");
        assert!(matches!(error.kind, HsErrorKind::NotFound(_)), "{error:?}");
    }

    Ok(())
}

/// A blob asked of no homeserver is read from the one its owner's record names.
#[tokio_shared_rt::test(shared)]
async fn test_a_blob_asked_of_no_homeserver_comes_from_the_one_the_record_names(
) -> Result<(), DynError> {
    let mut test = WatcherTest::setup(None).await?;
    let user_kp = Keypair::random();
    test.create_user(&user_kp, &profile("by record")).await?;
    let client = &test.event_processor_runner.client;
    let resource = PubkyResource::new(user_kp.public_key(), PROFILE_PATH)?;

    let blob = client.get_blob(None, &resource).await;

    test.cleanup_user(&user_kp).await?;
    assert_eq!(name_in(&blob?)?, "by record");

    Ok(())
}
