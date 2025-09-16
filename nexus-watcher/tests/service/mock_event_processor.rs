use crate::service::utils::{create_mock_event_processors, MockEventProcessorFactory, HS_IDS};
use nexus_common::types::DynError;
use nexus_watcher::service::TEventProcessorFactory;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(2);

#[tokio_shared_rt::test(shared)]
async fn test_mock_event_processors() -> Result<(), DynError> {
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let mock_processors = create_mock_event_processors(Some(TIMEOUT), shutdown_rx.clone());
    let factory = MockEventProcessorFactory::new(mock_processors, shutdown_rx);

    // Test successful event processor
    let processor = factory.build(HS_IDS[0].to_string()).await?;
    assert!(processor.run().await.is_ok());

    // Test error event processor
    let processor = factory.build(HS_IDS[1].to_string()).await?;
    assert!(processor.run().await.is_err());

    // Test panic event processor
    let processor = factory.build(HS_IDS[2].to_string()).await?;
    let res = processor.run().await;
    assert!(res.is_err() && res.unwrap_err().is_panic());

    // Test timeout scenarios
    let processor = factory.build(HS_IDS[3].to_string()).await?;
    match processor.run().await {
        Ok(_) => return Err(format!("Event processor should timeout after {TIMEOUT:?}s"))?,
        Err(_) => {}
    };

    let processor = factory.build(HS_IDS[4].to_string()).await?;
    match processor.run().await {
        Ok(_) => {}
        Err(_) => return Err(format!("Event processor should not timeout"))?,
    };

    Ok(())
}
