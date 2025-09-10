mod processor;
mod processor_factory;
mod result;
mod setup;

pub use processor::MockEventProcessor;
pub use processor_factory::MockEventProcessorFactory;
pub use setup::setup;

use nexus_common::models::homeserver::Homeserver;
use pubky::Keypair;
use pubky_app_specs::PubkyId;
pub use result::MockEventProcessorResult;
use std::{collections::HashMap, time::Duration};
use tokio::sync::watch::Receiver;

/// Create a success result type
pub fn success_result(message: &str) -> MockEventProcessorResult {
    MockEventProcessorResult::Success(message.to_string())
}

/// Create an error result type
pub fn error_result(message: &str) -> MockEventProcessorResult {
    MockEventProcessorResult::Error(message.to_string().into())
}

/// Create a panic result type
pub fn _panic_result() -> MockEventProcessorResult {
    MockEventProcessorResult::Panic()
}

// Create a random homeserver and add it to the event processor hashmap
pub async fn create_random_homeservers_and_persist(
    event_processor_hashmap: &mut HashMap<String, MockEventProcessor>,
    timeout: Option<Duration>,
    processor_status: MockEventProcessorResult,
    shutdown_rx: Receiver<bool>,
) {
    let homeserver_keypair = Keypair::random();
    let homeserver_public_key = homeserver_keypair.public_key().to_z32();

    let config_hs = PubkyId::try_from(homeserver_public_key.as_str()).unwrap();
    Homeserver::persist_if_unknown(config_hs).await.unwrap();

    let event_processor = MockEventProcessor {
        homeserver_id: homeserver_public_key.clone(),
        timeout,
        processor_status,
        shutdown_rx,
    };
    event_processor_hashmap.insert(homeserver_public_key.clone(), event_processor);
}
