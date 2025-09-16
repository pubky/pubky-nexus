use crate::service::utils::{
    create_random_homeservers_and_persist, error_result, setup, success_result,
    MockEventProcessorFactory,
};
use anyhow::Result;
use nexus_watcher::service::TEventProcessorFactory;
use std::time::Duration;

#[tokio_shared_rt::test(shared)]
async fn test_multiple_homeserver_event_processing() -> Result<()> {
    // Initialize the test
    let mut event_processor_list = setup().await?;
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Create 3 random homeservers with success result
    for _ in 0..3 {
        let processor_status = success_result("success from homeserver");
        create_random_homeservers_and_persist(
            &mut event_processor_list,
            None,
            processor_status,
            None,
            shutdown_rx.clone(),
        )
        .await;
    }

    // Create 1 random homeserver with error result
    let processor_status = error_result("PubkyClient: timeout from homeserver");
    create_random_homeservers_and_persist(
        &mut event_processor_list,
        None,
        processor_status,
        None,
        shutdown_rx.clone(),
    )
    .await;

    let factory = MockEventProcessorFactory::new(event_processor_list, shutdown_rx);

    let result = factory.run_all().await.unwrap();

    assert_eq!(result.count_ok, 3);
    assert_eq!(result.count_error, 1);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_multi_hs_event_processing_with_timeout() -> Result<()> {
    const EVENT_PROCESSOR_TIMEOUT: Option<Duration> = Some(Duration::from_secs(1));
    // Initialize the test
    let mut event_processor_list = setup().await?;
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Create 3 random homeservers with timeout limit
    for index in 0..3 {
        let processor_status = success_result("success from homeserver");
        create_random_homeservers_and_persist(
            &mut event_processor_list,
            Some(Duration::from_secs(index * 2)),
            processor_status,
            EVENT_PROCESSOR_TIMEOUT.clone(),
            shutdown_rx.clone(),
        )
        .await;
    }

    let factory = MockEventProcessorFactory::new(event_processor_list, shutdown_rx);

    let result = factory.run_all().await.unwrap();

    assert_eq!(result.count_ok, 1); // 1 success
    assert_eq!(result.count_timeout, 2); // 2 failures due to timeout

    Ok(())
}

