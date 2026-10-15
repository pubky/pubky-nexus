use crate::service::utils::{create_mock_event_processors, MockEventProcessorRunner, HS_IDS};
use anyhow::Result;
use nexus_common::types::DynError;
use nexus_watcher::service::TEventProcessorRunner;
use std::time::Duration;

const SLOW_RUN_THRESHOLD: Duration = Duration::from_secs(2);

#[tokio_shared_rt::test(shared)]
async fn test_mock_event_processors() -> Result<(), DynError> {
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let mock_processors =
        create_mock_event_processors(Some(SLOW_RUN_THRESHOLD), shutdown_rx.clone());
    let runner = MockEventProcessorRunner::new(mock_processors, HS_IDS.len(), shutdown_rx);

    // Test successful event processor
    let ev_processor_0 = runner.build(HS_IDS[0]).await?;
    assert!(ev_processor_0.run().await.is_ok());

    // Test error event processor
    let ev_processor_1 = runner.build(HS_IDS[1]).await?;
    assert!(ev_processor_1.run().await.is_err());

    // Test panic event processor
    let ev_processor_2 = runner.build(HS_IDS[2]).await?;
    let ev_processor_2_res = ev_processor_2.run().await;
    assert!(ev_processor_2_res.is_err() && ev_processor_2_res.unwrap_err().is_panic());

    // A slow run is waited for, and succeeds
    let ev_processor_3 = runner.build(HS_IDS[3]).await?;
    assert!(
        ev_processor_3.run().await.is_ok(),
        "A slow event processor run should be waited for"
    );

    let ev_processor_4 = runner.build(HS_IDS[4]).await?;
    assert!(ev_processor_4.run().await.is_ok());

    Ok(())
}
