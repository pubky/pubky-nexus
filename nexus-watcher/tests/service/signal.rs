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

    // Create 3 random homeservers with timeout limit
    for index in 0..3 {
        let processor_status = success_result("success from homeserver");
        create_random_homeservers_and_persist(&mut event_processor_hashmap, Some(Duration::from_secs(index * 2)), processor_status)
            .await;
    }

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let factory = MockEventProcessorFactory::new(
        event_processor_hashmap,
        None,
        shutdown_rx,
    );

    // Schedule Ctrl-C simulation after 1s
    tokio::spawn({
        let shutdown_tx = shutdown_tx.clone();
        async move {
            sleep(Duration::from_secs(1)).await;
            // simulate Ctrl-C
            let _ = shutdown_tx.send(true);
        }
    });

    let result = factory.run_all().await.unwrap();

    assert_eq!(result.0, 1); // 1 processor succeeds, because it finishes before the signal
    assert_eq!(result.1, 2); // 2 processors fail, because the signal is received before they finish

    Ok(())
}
