use std::sync::Arc;

use crate::service::utils::{MockEventProcessorResult, HS_IDS};
use nexus_common::models::event::EventProcessorError;
use nexus_common::models::homeserver::Homeserver;
use nexus_watcher::service::TEventProcessor;
use pubky::Keypair;
use pubky_app_specs::PubkyId;
use tokio::sync::watch::Receiver;
use tokio::time::Duration;

pub struct MockEventProcessor {
    pub homeserver_id: PubkyId,
    /// Desired event processor status. In other words, the type of execution that `run` should simulate.
    processor_status: MockEventProcessorResult,
    /// If set, this mock processor will return successfully after waiting for this amount of time
    sleep_duration: Option<Duration>,
    custom_timeout: Option<Duration>,
    shutdown_rx: Receiver<bool>,
}

#[async_trait::async_trait]
impl TEventProcessor for MockEventProcessor {
    fn get_homeserver_id(&self) -> PubkyId {
        self.homeserver_id.clone()
    }

    fn custom_timeout(&self) -> Option<Duration> {
        self.custom_timeout
    }

    async fn run_internal(self: Arc<Self>) -> Result<(), EventProcessorError> {
        // Simulate a long-running task if needed, but be responsive to shutdown
        // This simulates the processing of event lines, which can take a while but can be interrupted by the shutdown signal
        if let Some(sleep_duration) = self.sleep_duration {
            let mut shutdown_rx = self.shutdown_rx.clone();
            tokio::select! {
                _ = tokio::time::sleep(sleep_duration) => {},
                _ = shutdown_rx.changed() => {
                    return Ok(());
                }
            }
        }

        match &self.processor_status {
            MockEventProcessorResult::Success => Ok(()),
            MockEventProcessorResult::Error(e) => Err(EventProcessorError::Generic(e.clone())),
            MockEventProcessorResult::Panic => panic!("Event processor panicked: unknown error"),
        }
    }
}

/// Create a random homeserver and add it to the event processor list
pub async fn create_random_homeservers_and_persist(
    event_processor_list: &mut Vec<MockEventProcessor>,
    sleep_duration: Option<Duration>,
    processor_status: MockEventProcessorResult,
    custom_timeout: Option<Duration>,
    shutdown_rx: Receiver<bool>,
) {
    let homeserver_keypair = Keypair::random();
    let homeserver_public_key = homeserver_keypair.public_key().to_z32();

    let homeserver_id = PubkyId::try_from(homeserver_public_key.as_str()).unwrap();
    Homeserver::persist_if_unknown(homeserver_id.clone())
        .await
        .unwrap();

    let event_processor = MockEventProcessor {
        homeserver_id,
        sleep_duration,
        processor_status,
        custom_timeout,
        shutdown_rx,
    };
    event_processor_list.push(event_processor);
}

/// Create a list of mock event processors
pub fn create_mock_event_processors(
    custom_timeout: Option<Duration>,
    shutdown_rx: Receiver<bool>,
) -> Vec<MockEventProcessor> {
    use MockEventProcessorResult::*;
    [
        (HS_IDS[0], None, Success),
        (HS_IDS[1], None, Error("Event processor error!".into())),
        (HS_IDS[2], None, Panic),
        (HS_IDS[3], Some(3), Success),
        (HS_IDS[4], Some(1), Success),
    ]
    .into_iter()
    .map(
        |(homeserver_id, sleep_duration_sec, status)| MockEventProcessor {
            homeserver_id: PubkyId::try_from(homeserver_id).unwrap(),
            sleep_duration: sleep_duration_sec.map(Duration::from_secs),
            processor_status: status,
            custom_timeout,
            shutdown_rx: shutdown_rx.clone(),
        },
    )
    .collect()
}
