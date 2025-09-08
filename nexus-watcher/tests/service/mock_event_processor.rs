use crate::service::utils::{MockEventProcessor, MockEventProcessorFactory, MockEventProcessorResult};
use anyhow::{anyhow, Result};
use nexus_watcher::events::TEventProcessorFactory;
use std::{collections::HashMap, time::Duration};
use tokio::time::timeout;

const HOMESERVER_IDS: [&str; 5] = [
    "1hb71xx9km3f4pw5izsy1gn19ff1uuuqonw4mcygzobwkryujoiy",
    "8rsrmfrn1anbrzuxiffwy1174o58emf4qgbfk5h7s8a33r3bd8dy",
    "984orjzbusofbqhsqz9axpez3uuwd3hbpqztd6rtx3pr78y9s1my",
    "mamtihagiptrngan9y6cdj1xu7yb8yc7us9uerytaewc13ejqy9y",
    "8x93apuue6kjyqosu1wp9xye45j9noq8y3pmuwmhfo3o95eimgoo",
];

const EVENT_PROCESSOR_TIMEOUT: Duration = Duration::from_secs(2);

#[tokio_shared_rt::test(shared)]
async fn test_mock_event_processors() -> Result<()> {
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let factory =
        MockEventProcessorFactory::new(create_mock_event_processors(), Some(EVENT_PROCESSOR_TIMEOUT), shutdown_rx);
        

    // Test successful event processor
    simulate_event_processor_success(&factory, HOMESERVER_IDS[0]).await?;

    // Test error event processor
    simulate_event_processor_error(&factory, HOMESERVER_IDS[1]).await?;

    // Test panic event processor
    simulate_event_processor_panic(&factory, HOMESERVER_IDS[2]).await?;

    // Test timeout scenarios
    simulate_event_processor_timeout(&factory, HOMESERVER_IDS[3]).await?; 
    simulate_event_processor_completes_within_timeout(&factory, HOMESERVER_IDS[4]).await?;

    Ok(())
}

async fn simulate_event_processor_success(
    factory: &MockEventProcessorFactory,
    homeserver_id: &str,
) -> Result<()> {
    let processor = factory
        .build(homeserver_id.to_string())
        .await
        .map_err(|e| anyhow!(e))?;
    assert!(processor.run(factory.shutdown_rx()).await.is_ok());
    Ok(())
}

async fn simulate_event_processor_error(
    factory: &MockEventProcessorFactory,
    homeserver_id: &str,
) -> Result<()> {
    let processor = factory
        .build(homeserver_id.to_string())
        .await
        .map_err(|e| anyhow!(e))?;
    assert!(processor.run(factory.shutdown_rx()).await.is_err());
    Ok(())
}

/// Tests that the system can gracefully handle processor panics without crashing the entire application
async fn simulate_event_processor_panic(
    factory: &MockEventProcessorFactory,
    homeserver_id: &str,
) -> Result<()> {
    let processor = factory
        .build(homeserver_id.to_string())
        .await
        .map_err(|e| anyhow!(e))?;
    let shutdown_rx = factory.shutdown_rx();
    // We use `tokio::spawn` to isolate the panic - without it, the panic would propagate up and crash the test.
    // The `JoinHandle` allows us to detect that a panic occurred via `is_panic()` on the join error.
    let join_result = tokio::spawn(async move { processor.run(shutdown_rx).await }).await;
    assert!(join_result.is_err());
    assert!(join_result.unwrap_err().is_panic());
    Ok(())
}

async fn simulate_event_processor_timeout(
    factory: &MockEventProcessorFactory,
    homeserver_id: &str,
) -> Result<()> {
    let processor = factory
        .build(homeserver_id.to_string())
        .await
        .map_err(|e| anyhow!(e))?;
    match timeout(factory.timeout(), processor.run(factory.shutdown_rx())).await {
        Ok(_) => Err(anyhow!(
            "Event processor should timeout after {EVENT_PROCESSOR_TIMEOUT:?}s"
        )),
        Err(_) => Ok(()), // expected timeout
    }
}

async fn simulate_event_processor_completes_within_timeout(
    factory: &MockEventProcessorFactory,
    homeserver_id: &str,
) -> Result<()> {
    let processor = factory
        .build(homeserver_id.to_string())
        .await
        .map_err(|e| anyhow!(e))?;
    match timeout(factory.timeout(), processor.run(factory.shutdown_rx())).await {
        Ok(_) => Ok(()),
        Err(_) => Err(anyhow!("Event processor should not timeout")),
    }
}

fn create_mock_event_processors() -> HashMap<String, MockEventProcessor> {
    use MockEventProcessorResult::*;

    let processors = [
        (
            HOMESERVER_IDS[0],
            None,
            Success("Success finished!".to_string()),
        ),
        (
            HOMESERVER_IDS[1],
            None,
            Error("Event processor error!".to_string().into()),
        ),
        (HOMESERVER_IDS[2], None, Panic()),
        (
            HOMESERVER_IDS[3],
            Some(Duration::from_secs(3)),
            Success("Success finished!".to_string()),
        ),
        (
            HOMESERVER_IDS[4],
            Some(Duration::from_secs(1)),
            Success("Success finished!".to_string()),
        ),
    ];

    processors
        .into_iter()
        .map(|(id, timeout, status)| {
            let processor = MockEventProcessor {
                homeserver_id: id.to_string(),
                timeout,
                processor_status: status,
            };
            (id.to_string(), processor)
        })
        .collect()
}
