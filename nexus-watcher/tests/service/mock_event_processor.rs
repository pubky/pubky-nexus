use crate::service::utils::{
    MockEventProcessor, MockEventProcessorFactory, MockEventProcessorResult,
};
use nexus_common::types::DynError;
use nexus_watcher::events::TEventProcessorFactory;
use std::{collections::HashMap, time::Duration};
use tokio::{sync::watch::Receiver, time::timeout};

const HS_IDS: [&str; 5] = [
    "1hb71xx9km3f4pw5izsy1gn19ff1uuuqonw4mcygzobwkryujoiy",
    "8rsrmfrn1anbrzuxiffwy1174o58emf4qgbfk5h7s8a33r3bd8dy",
    "984orjzbusofbqhsqz9axpez3uuwd3hbpqztd6rtx3pr78y9s1my",
    "mamtihagiptrngan9y6cdj1xu7yb8yc7us9uerytaewc13ejqy9y",
    "8x93apuue6kjyqosu1wp9xye45j9noq8y3pmuwmhfo3o95eimgoo",
];

const TIMEOUT: Duration = Duration::from_secs(2);

#[tokio_shared_rt::test(shared)]
async fn test_mock_event_processors() -> Result<(), DynError> {
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let mock_processors = create_mock_event_processors(shutdown_rx.clone());
    let factory = MockEventProcessorFactory::new(mock_processors, Some(TIMEOUT));

    // Test successful event processor
    let processor = factory.build(HS_IDS[0].to_string()).await?;
    assert!(processor.run().await.is_ok());

    // Test error event processor
    let processor = factory.build(HS_IDS[1].to_string()).await?;
    assert!(processor.run().await.is_err());

    // Test panic event processor
    let processor = factory.build(HS_IDS[2].to_string()).await?;
    let join_result = tokio::spawn(async move { processor.run().await }).await;
    assert!(join_result.is_err() && join_result.unwrap_err().is_panic());

    // Test timeout scenarios
    let processor = factory.build(HS_IDS[3].to_string()).await?;
    match timeout(factory.timeout(), processor.run()).await {
        Ok(_) => return Err(format!("Event processor should timeout after {TIMEOUT:?}s"))?,
        Err(_) => {}
    };

    let processor = factory.build(HS_IDS[4].to_string()).await?;
    match timeout(factory.timeout(), processor.run()).await {
        Ok(_) => {}
        Err(_) => return Err(format!("Event processor should not timeout"))?,
    };

    Ok(())
}

fn create_mock_event_processors(
    shutdown_rx: Receiver<bool>,
) -> HashMap<String, MockEventProcessor> {
    use MockEventProcessorResult::*;
    [
        (HS_IDS[0], None, Success("Success finished!".into())),
        (HS_IDS[1], None, Error("Event processor error!".into())),
        (HS_IDS[2], None, Panic()),
        (HS_IDS[3], Some(3), Success("Success finished!".into())),
        (HS_IDS[4], Some(1), Success("Success finished!".into())),
    ]
    .into_iter()
    .map(|(id, sleep_duration_sec, status)| {
        let processor = MockEventProcessor {
            homeserver_id: id.to_string(),
            sleep_duration: sleep_duration_sec.map(Duration::from_secs),
            processor_status: status,
            shutdown_rx: shutdown_rx.clone(),
        };
        (id.to_string(), processor)
    })
    .collect()
}
