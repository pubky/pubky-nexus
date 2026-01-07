use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky::PublicKey;

pub async fn create_external_test_homeserver(test: &mut WatcherTest) -> Result<PublicKey> {
    let mut testnet_guard = test.testnet.lock().await;
    let homeserver_id = testnet_guard.create_random_homeserver().await?.public_key();
    Ok(homeserver_id)
}
