use crate::service::utils::{create_mock_event_processors, MockEventProcessorRunner, HS_IDS};
use nexus_common::types::DynError;
use nexus_watcher::service::TEventProcessorRunner;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(2);

#[tokio_shared_rt::test(shared)]
async fn test_mock_event_processors() -> Result<(), DynError> {
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let mock_processors = create_mock_event_processors(Some(TIMEOUT), shutdown_rx.clone());
    let runner = MockEventProcessorRunner::new(mock_processors, HS_IDS.len(), shutdown_rx);

    // Test successful event processor
    let ev_processor_0 = runner.build(HS_IDS[0].to_string()).await?;
    assert!(ev_processor_0.run().await.is_ok());

    // Test error event processor
    let ev_processor_1 = runner.build(HS_IDS[1].to_string()).await?;
    assert!(ev_processor_1.run().await.is_err());

    // Test panic event processor
    let ev_processor_2 = runner.build(HS_IDS[2].to_string()).await?;
    let ev_processor_2_res = ev_processor_2.run().await;
    assert!(ev_processor_2_res.is_err() && ev_processor_2_res.unwrap_err().is_panic());

    // Test timeout scenarios
    let ev_processor_3 = runner.build(HS_IDS[3].to_string()).await?;
    let ev_processor_3_res = ev_processor_3.run().await;
    assert!(ev_processor_3_res.is_err() && ev_processor_3_res.unwrap_err().is_timeout());

    let ev_processor_4 = runner.build(HS_IDS[4].to_string()).await?;
    assert!(
        ev_processor_4.run().await.is_ok(),
        "Event processor should not timeout"
    );

    Ok(())
}
