use crate::event_processor::utils::default_moderation_tests;
use crate::service::utils::HS_IDS;
use crate::service::utils::{create_mock_event_processors, setup, MockEventProcessorFactory};
use anyhow::Result;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use nexus_watcher::service::EventProcessorFactory;
use nexus_watcher::service::TEventProcessorFactory;
use pubky_app_specs::PubkyId;
use std::path::PathBuf;
use std::sync::Arc;

#[tokio_shared_rt::test(shared)]
async fn test_event_processor_factory_default_homeserver_prioritization() -> Result<(), DynError> {
    // Initialize the test
    setup().await?;

    let factory = EventProcessorFactory {
        default_homeserver: PubkyId::try_from(HS_IDS[3]).unwrap(),
        shutdown_rx: tokio::sync::watch::channel(false).1,
        limit: 1000,
        files_path: PathBuf::from("/tmp/nexus-watcher-test"),
        tracer_name: String::from("unit-test-hs-list-test"),
        moderation: Arc::new(default_moderation_tests()),
    };

    // Persist the homeservers
    for hs_id in HS_IDS {
        let hs = Homeserver::new(PubkyId::try_from(hs_id).unwrap());
        hs.put_to_graph().await.unwrap();
    }

    // Prioritize the default homeserver
    let hs_ids = factory.prioritize_default_homeserver().await;
    assert_eq!(hs_ids[0], HS_IDS[3]);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_mock_event_processor_factory_default_homeserver_prioritization(
) -> Result<(), DynError> {
    // Initialize the test
    setup().await?;

    let event_processors = create_mock_event_processors(tokio::sync::watch::channel(false).1)
        .into_iter()
        .map(|processor| Arc::new(processor))
        .collect();

    let factory = MockEventProcessorFactory {
        event_processors,
        timeout: None,
        shutdown_rx: tokio::sync::watch::channel(false).1,
    };

    // Persist the homeservers
    for hs_id in HS_IDS {
        let hs = Homeserver::new(PubkyId::try_from(hs_id).unwrap());
        hs.put_to_graph().await.unwrap();
    }

    // Prioritize the default homeserver
    let hs_ids = factory.prioritize_default_homeserver().await;
    assert_eq!(hs_ids[0], HS_IDS[0]);

    Ok(())
}
