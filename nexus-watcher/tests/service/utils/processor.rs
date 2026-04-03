use std::path::PathBuf;
use std::sync::Arc;

use crate::service::utils::{MockEventProcessorResult, HS_IDS};
use chrono::Utc;
use nexus_common::db::exec_single_row;
use nexus_common::db::queries;
use nexus_common::models::event::EventProcessorError;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::models::user::UserDetails;
use nexus_watcher::events::Moderation;
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
    files_path: PathBuf,
    moderation: Arc<Moderation>,
}

#[async_trait::async_trait]
impl TEventProcessor for MockEventProcessor {
    fn files_path(&self) -> &PathBuf {
        &self.files_path
    }

    fn moderation(&self) -> &Arc<Moderation> {
        &self.moderation
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

fn default_mock_moderation() -> Arc<Moderation> {
    Arc::new(Moderation {
        id: PubkyId::try_from("8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo").unwrap(),
        tags: vec![],
    })
}

/// Create a random homeserver and add it to the event processor list.
///
/// If `create_active_users` is `Some(n)`, `n` test users will be created in the
/// graph and linked to this homeserver via `HOSTED_BY`.
pub async fn create_random_homeservers_and_persist(
    event_processor_list: &mut Vec<MockEventProcessor>,
    sleep_duration: Option<Duration>,
    processor_status: MockEventProcessorResult,
    custom_timeout: Option<Duration>,
    shutdown_rx: Receiver<bool>,
    create_active_users: Option<u64>,
) {
    let homeserver_keypair = Keypair::random();
    let homeserver_public_key = homeserver_keypair.public_key().to_z32();

    let homeserver_id = PubkyId::try_from(homeserver_public_key.as_str()).unwrap();
    Homeserver::persist_if_unknown(homeserver_id.clone())
        .await
        .unwrap();

    // Create test users linked to this homeserver via HOSTED_BY
    if let Some(count) = create_active_users {
        for _ in 0..count {
            let user_keypair = Keypair::random();
            let user_id = PubkyId::try_from(user_keypair.public_key().to_z32().as_str()).unwrap();
            let user = UserDetails {
                id: user_id.clone(),
                name: "test-user".to_string(),
                bio: None,
                status: None,
                links: None,
                image: None,
                indexed_at: Utc::now().timestamp_millis(),
            };
            let create_query = queries::put::create_user(&user).unwrap();
            exec_single_row(create_query).await.unwrap();
            let link_query = queries::put::set_user_homeserver(&user_id, &homeserver_id);
            exec_single_row(link_query).await.unwrap();
        }
    }

    let event_processor = MockEventProcessor {
        homeserver_id,
        sleep_duration,
        processor_status,
        custom_timeout,
        shutdown_rx,
        files_path: PathBuf::from("/tmp/mock"),
        moderation: default_mock_moderation(),
    };
    event_processor_list.push(event_processor);
}

/// Create a list of mock event processors
pub fn create_mock_event_processors(
    custom_timeout: Option<Duration>,
    shutdown_rx: Receiver<bool>,
) -> Vec<MockEventProcessor> {
    use MockEventProcessorResult::*;
    let moderation = default_mock_moderation();
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
            files_path: PathBuf::from("/tmp/mock"),
            moderation: moderation.clone(),
        },
    )
    .collect()
}
