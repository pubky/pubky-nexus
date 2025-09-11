use crate::service::utils::{
    create_random_homeservers_and_persist, setup, success_result, MockEventProcessorFactory,
};
use anyhow::Result;
use nexus_watcher::events::TEventProcessorFactory;
use std::time::Duration;
use tokio::time::sleep;

#[tokio_shared_rt::test(shared)]
async fn test_shutdown_signal() -> Result<()> {
    // Initialize the test
    let mut event_processor_hashmap = setup().await?;
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

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

    let factory = MockEventProcessorFactory::new(event_processor_hashmap, None, shutdown_rx);

    // Schedule Ctrl-C simulation after 1s
    tokio::spawn({
        let shutdown_tx = shutdown_tx.clone();
        async move {
            sleep(Duration::from_secs(1)).await;
            let _ = shutdown_tx.send(true);
        }
    });

    let result = factory.run_all().await.unwrap();

    // We created 3 HSs, each with different execution durations (0s, 2s, 4s)
    // We triggered the shutdown signal 1s after start
    assert_eq!(result.0, 2); // 2 processors run without errors (of the 3, the 3rd one didn't even start)
    assert_eq!(result.1, 0); // no processors fail, because no erratic or unexpected behavior was triggered

    Ok(())
}
