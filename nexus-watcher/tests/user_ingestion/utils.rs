use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::db::kv::RedisOps;
use nexus_common::models::user::{UserDetails, USER_HS_CURSOR};
use pubky::PublicKey;

pub async fn create_external_test_homeserver(test: &mut WatcherTest) -> Result<PublicKey> {
    let homeserver_id = test.testnet.create_random_homeserver().await?.public_key();
    Ok(homeserver_id)
}

/// Asserts that a user was properly ingested: graph node exists and the
/// `USER_HS_CURSOR` sorted-set entry points to the expected homeserver.
pub async fn assert_user_ingested(user_id: &str, hs_pk: &PublicKey) {
    let hs_id = hs_pk.to_z32();

    let user = UserDetails::get_by_id(user_id)
        .await
        .expect("UserDetails::get_by_id failed");
    assert!(user.is_some(), "User {user_id} should be ingested in graph/cache");

    let cursor_key = [&USER_HS_CURSOR[..], &[user_id]].concat();
    let cursor = UserDetails::check_sorted_set_member(None, &cursor_key, &[&hs_id])
        .await
        .expect("check_sorted_set_member failed");
    assert!(
        cursor.is_some(),
        "USER_HS_CURSOR should map user {user_id} to homeserver {hs_id}"
    );
}
