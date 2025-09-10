use crate::service::utils::{
    create_random_homeservers_and_persist, error_result, setup, success_result,
    MockEventProcessorFactory,
};
use anyhow::Result;
use nexus_common::models::homeserver::Homeserver;
use nexus_watcher::events::TEventProcessorFactory;
use std::time::Duration;

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

    let factory = MockEventProcessorFactory::new(event_processor_hashmap, None);

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

    let factory = MockEventProcessorFactory::new(event_processor_hashmap, EVENT_PROCESSOR_TIMEOUT);

    let result = factory.run_all().await.unwrap();

    assert_eq!(result.0, 1); // 1 success
    assert_eq!(result.1, 2); // 2 failures due to timeout

    Ok(())
}

// TODO We need a way to ensure this test finds no homeservers in the graph
#[tokio_shared_rt::test(shared)]
async fn test_no_hs_found() -> Result<()> {
    let event_processor_hashmap = setup().await?;

    let factory = MockEventProcessorFactory::new(event_processor_hashmap, None);

    // We explicitly don't add any homeservers, so we expect an empty list, which should result in an error
    assert!(Homeserver::get_all_from_graph().await.is_err());

    // Ensure that run_all does not panic or throw an error in case no HSs were found
    assert!(factory.run_all().await.is_ok());

    Ok(())
}
