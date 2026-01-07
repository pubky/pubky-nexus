use crate::event_processor::utils::watcher::SHARED_TESTNET;
use anyhow::{Error, Result};
use pubky::PublicKey;

pub async fn create_external_test_homeserver() -> Result<PublicKey> {
    let testnet = SHARED_TESTNET
        .get()
        .ok_or(Error::msg("SHARED_TESTNET not initialized"))?;
    let mut testnet_guard = testnet.lock().await;
    let homeserver_id = testnet_guard.create_random_homeserver().await?.public_key();
    Ok(homeserver_id)
}
