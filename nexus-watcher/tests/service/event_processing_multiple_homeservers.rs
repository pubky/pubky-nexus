use crate::{event_processor::utils::default_moderation_tests, service::utils::{
    create_random_homeservers_and_persist, error_result, setup, success_result,
    MockEventProcessorFactory, HS_IDS,
}};
use anyhow::Result;
use nexus_common::{models::homeserver::Homeserver, types::DynError};
use nexus_watcher::service::{EventProcessorFactory, TEventProcessorFactory};
use pubky_app_specs::PubkyId;
use std::{path::PathBuf, sync::Arc, time::Duration};

#[tokio_shared_rt::test(shared)]
async fn test_multiple_homeserver_event_processing() -> Result<()> {
    // Initialize the test
    let mut event_processor_hashmap = setup().await?;
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Create 3 random homeservers with success result
    for _ in 0..3 {
        let processor_status = success_result("success from homeserver");
        create_random_homeservers_and_persist(
            &mut event_processor_hashmap,
            None,
            processor_status,
            shutdown_rx.clone(),
        )
        .await;
    }

    // Create 1 random homeserver with error result
    let processor_status = error_result("PubkyClient: timeout from homeserver");
    create_random_homeservers_and_persist(
        &mut event_processor_hashmap,
        None,
        processor_status,
        shutdown_rx.clone(),
    )
    .await;

    let factory = MockEventProcessorFactory::new(event_processor_hashmap, None, shutdown_rx);

    let result = factory.run_all().await.unwrap();

    assert_eq!(result.0, 3);
    assert_eq!(result.1, 1);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_multi_hs_event_processing_with_timeout() -> Result<()> {
    const EVENT_PROCESSOR_TIMEOUT: Option<Duration> = Some(Duration::from_secs(1));
    // Initialize the test
    let mut event_processor_hashmap = setup().await?;
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Create 3 random homeservers with timeout limit
    for index in 0..3 {
        let processor_status = success_result("success from homeserver");
        create_random_homeservers_and_persist(
            &mut event_processor_hashmap,
            Some(Duration::from_secs(index * 2)),
            processor_status,
            shutdown_rx.clone(),
        )
        .await;
    }

    let factory = MockEventProcessorFactory::new(
        event_processor_hashmap,
        EVENT_PROCESSOR_TIMEOUT,
        shutdown_rx,
    );

    let result = factory.run_all().await.unwrap();

    assert_eq!(result.0, 1); // 1 success
    assert_eq!(result.1, 2); // 2 failures due to timeout

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_default_homeserver_prioritization() -> Result<(), DynError> {

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

