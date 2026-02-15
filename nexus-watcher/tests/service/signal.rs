use crate::service::utils::{
    create_random_homeservers_and_persist, setup, MockEventProcessorResult,
    MockEventProcessorRunner,
};
use anyhow::Result;
use nexus_watcher::service::TEventProcessorRunner;
use std::time::Duration;
use tokio::time::sleep;

#[tokio_shared_rt::test(shared)]
async fn test_shutdown_signal() -> Result<()> {
    // Initialize the test
    let mut event_processor_list = setup().await?;
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Create 3 random homeservers with timeout limit
    // Index 0: 0s sleep (default, excluded from run_all)
    // Index 1: 2s sleep
    // Index 2: 4s sleep
    for index in 0..3 {
        let processor_status = MockEventProcessorResult::Success;
        create_random_homeservers_and_persist(
            &mut event_processor_list,
            Some(Duration::from_secs(index * 2)),
            processor_status,
            None,
            shutdown_rx.clone(),
        )
        .await;
    }

    let runner = MockEventProcessorRunner::new(event_processor_list, 3, shutdown_rx);

    // Schedule Ctrl-C simulation after 1s
    tokio::spawn({
        let shutdown_tx = shutdown_tx.clone();
        async move {
            sleep(Duration::from_secs(1)).await;
            let _ = shutdown_tx.send(true);
        }
    });

    let stats = runner.run_all().await.unwrap().0;

    // run_all excludes the default HS (0s sleep).
    // Of the remaining 2 (2s, 4s sleep), the shutdown signal fires after 1s.
    // The 2s HS starts running, detects shutdown and exits early with Ok.
    // The 4s HS doesn't start because shutdown is detected before it begins.
    assert_eq!(stats.count_ok(), 1); // 1 processor exited gracefully
    assert_eq!(stats.count_error(), 0);
    assert_eq!(stats.count_panic(), 0);
    assert_eq!(stats.count_timeout(), 0);

    Ok(())
}
