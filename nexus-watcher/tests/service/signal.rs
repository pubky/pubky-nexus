use anyhow::Result;
use nexus_watcher::service::rolling_window::run_processors;
use crate::service::utils::{create_random_homeservers_and_persist, setup, success_result, MockEventProcessorFactory};
use std::{sync::Arc, time::Duration};
use tokio::time::sleep;

const EVENT_PROCESSOR_TIMEOUT: Option<Duration> = Some(Duration::from_secs(2));

#[tokio_shared_rt::test(shared)]
async fn test_shutdown_signal() -> Result<()> {
    // Initialize the test
    let mut event_processor_hashmap = setup().await?;

    // Create 3 random homeservers with timeout limit
    for index in 0..3 {
        let processor_status = success_result("success from homeserver");
        create_random_homeservers_and_persist(&mut event_processor_hashmap, Some(Duration::from_secs(index * 2)), processor_status).await;
    }

    let factory = MockEventProcessorFactory::new(event_processor_hashmap, EVENT_PROCESSOR_TIMEOUT);

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Schedule Ctrl-C simulation after 1s
    tokio::spawn({
        let shutdown_tx = shutdown_tx.clone();
        async move {
            sleep(Duration::from_secs(1)).await;
            let _ = shutdown_tx.send(true); // simulate Ctrl-C
        }
    });

    let result = run_processors(Arc::new(factory), shutdown_rx)
        .await
        .unwrap();
    
    assert_eq!(result.0, 1);
    assert_eq!(result.1, 2);

    Ok(())
}
