use crate::service::utils::{
    create_random_homeservers_and_persist, setup, MockEventProcessorResult,
    MockEventProcessorRunner,
};
use anyhow::Result;
use nexus_watcher::service::TEventProcessorRunner;
use std::time::Duration;

#[tokio_shared_rt::test(shared)]
async fn test_multiple_homeserver_event_processing() -> Result<()> {
    // Initialize the test
    let mut event_processor_list = setup().await?;
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Create 3 random homeservers with success result
    for _ in 0..3 {
        let processor_status = MockEventProcessorResult::Success;
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
    let processor_status = MockEventProcessorResult::Error("PubkyClient: timeout from HS".into());
    create_random_homeservers_and_persist(
        &mut event_processor_list,
        None,
        processor_status,
        None,
        shutdown_rx.clone(),
    )
    .await;

    let runner = MockEventProcessorRunner::new(event_processor_list, 4, shutdown_rx);

    let stats = runner.run_all().await.unwrap().0;
    assert_eq!(stats.count_ok(), 3);
    assert_eq!(stats.count_error(), 1);
    assert_eq!(stats.count_panic(), 0);
    assert_eq!(stats.count_timeout(), 0);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_multi_hs_event_processing_with_homeserver_limit() -> Result<()> {
    // Initialize the test
    let mut event_processor_list = setup().await?;
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Create 5 random homeservers
    for _ in 0..5 {
        let processor_status = MockEventProcessorResult::Success;
        create_random_homeservers_and_persist(
            &mut event_processor_list,
            None,
            processor_status,
            None,
            shutdown_rx.clone(),
        )
        .await;
    }

    assert_eq!(event_processor_list.len(), 5); // Ensure 5 HSs are available
    let hs_limit = 3; // Configure a monitored_homeservers_limit of 3
    let runner = MockEventProcessorRunner::new(event_processor_list, hs_limit, shutdown_rx);
    let stats = runner.run_all().await.unwrap().0;

    assert_eq!(stats.count_ok(), 3); // 3 successful ones, due to the limit (5 HSs were available)
    assert_eq!(stats.count_timeout(), 0);
    assert_eq!(stats.count_error(), 0);
    assert_eq!(stats.count_panic(), 0);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_multi_hs_event_processing_with_homeserver_limit_one() -> Result<()> {
    // Initialize the test
    let mut event_processor_list = setup().await?;
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Create 5 random homeservers
    for _ in 0..5 {
        let processor_status = MockEventProcessorResult::Success;
        create_random_homeservers_and_persist(
            &mut event_processor_list,
            None,
            processor_status,
            None,
            shutdown_rx.clone(),
        )
        .await;
    }

    assert_eq!(event_processor_list.len(), 5); // Ensure 5 HSs are available

    // Check that, when the limit is 1, only the default (first) homeserver is considered
    let runner_one = MockEventProcessorRunner::new(event_processor_list, 1, shutdown_rx);
    let hs_list = runner_one.pre_run_all().await.unwrap();
    assert_eq!(hs_list.len(), 1);
    assert_eq!(hs_list.first().unwrap(), &runner_one.default_homeserver());

    let stats_one = runner_one.run_all().await.unwrap().0;
    assert_eq!(stats_one.count_ok(), 1); // 1 successful, due to the limit (5 HSs were available)
    assert_eq!(stats_one.count_timeout(), 0);
    assert_eq!(stats_one.count_error(), 0);
    assert_eq!(stats_one.count_panic(), 0);

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
        let processor_status = MockEventProcessorResult::Success;
        create_random_homeservers_and_persist(
            &mut event_processor_list,
            Some(Duration::from_secs(index * 2)),
            processor_status,
            EVENT_PROCESSOR_TIMEOUT,
            shutdown_rx.clone(),
        )
        .await;
    }

    let runner = MockEventProcessorRunner::new(event_processor_list, 3, shutdown_rx);

    let stats = runner.run_all().await.unwrap().0;
    assert_eq!(stats.count_ok(), 1); // 1 success
    assert_eq!(stats.count_timeout(), 2); // 2 failures due to timeout
    assert_eq!(stats.count_error(), 0);
    assert_eq!(stats.count_panic(), 0);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_multi_hs_event_processing_with_panic() -> Result<()> {
    // Initialize the test
    let mut event_processor_list = setup().await?;
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Create 3 random homeservers expected to succeed
    for _i in 0..3 {
        let processor_status = MockEventProcessorResult::Success;
        create_random_homeservers_and_persist(
            &mut event_processor_list,
            None,
            processor_status,
            None,
            shutdown_rx.clone(),
        )
        .await;
    }

    // Create 2 random homeservers expected to panic
    for _i in 0..2 {
        let processor_status = MockEventProcessorResult::Panic;
        create_random_homeservers_and_persist(
            &mut event_processor_list,
            None,
            processor_status,
            None,
            shutdown_rx.clone(),
        )
        .await;
    }

    let runner = MockEventProcessorRunner::new(event_processor_list, 5, shutdown_rx);

    let stats = runner.run_all().await.unwrap().0;
    assert_eq!(stats.count_ok(), 3); // 3 expected to succeed
    assert_eq!(stats.count_timeout(), 0);
    assert_eq!(stats.count_error(), 0);
    assert_eq!(stats.count_panic(), 2); // 2 expected to panic

    Ok(())
}
