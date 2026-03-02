use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky::PublicKey;

pub async fn create_external_test_homeserver(test: &mut WatcherTest) -> Result<PublicKey> {
    let homeserver_id = test.testnet.create_random_homeserver().await?.public_key();
    Ok(homeserver_id)
}
