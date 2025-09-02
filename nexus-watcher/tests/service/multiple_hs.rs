use crate::service::utils::{MockEventProcessor, MockEventProcessorFactory};
use anyhow::{anyhow, Result};
use nexus_watcher::{service::ProcessResult, TEventProcessorFactory};
use tokio::time::timeout;
use std::{collections::HashMap, time::Duration};

const HOMESERVER_IDS: [&str; 4] = [
    "1hb71xx9km3f4pw5izsy1gn19ff1uuuqonw4mcygzobwkryujoiy",
    "8rsrmfrn1anbrzuxiffwy1174o58emf4qgbfk5h7s8a33r3bd8dy",
    "984orjzbusofbqhsqz9axpez3uuwd3hbpqztd6rtx3pr78y9s1my",
    "mamtihagiptrngan9y6cdj1xu7yb8yc7us9uerytaewc13ejqy9y",
];

#[tokio_shared_rt::test(shared)]
async fn test_multiple_mock_event_processors() -> Result<()> {
    let event_processor_hashmap = create_mock_event_processors();
    let factory = MockEventProcessorFactory::new(event_processor_hashmap);
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Run successful event proccessor
    let processor = factory
        .build(HOMESERVER_IDS[0].to_string())
        .await
        .map_err(|e| anyhow!(e))?;
    assert!(processor.run(shutdown_rx.clone()).await.is_ok());

    // Run error event proccessor
    let processor = factory
        .build(HOMESERVER_IDS[1].to_string())
        .await
        .map_err(|e| anyhow!(e))?;
    assert!(processor.run(shutdown_rx.clone()).await.is_err());

    // Run panic event proccessor
    let processor = factory
        .build(HOMESERVER_IDS[2].to_string())
        .await
        .map_err(|e| anyhow!(e))?;
    let shutdown_rx2 = shutdown_rx.clone();
    let join_result = tokio::spawn(async move { processor.run(shutdown_rx2).await }).await;
    assert!(join_result.is_err());
    assert!(join_result.unwrap_err().is_panic());

    // Run timeout event proccessor
    let processor = factory
        .build(HOMESERVER_IDS[3].to_string())
        .await
        .map_err(|e| anyhow!(e))?;
    match timeout(Duration::from_secs(3), processor.run(shutdown_rx.clone())).await {
        Ok(_) => {}
        Err(_) => panic!("Event processor should nottimeout")
    }

    // Run no timeout event proccessor
    let processor = factory
        .build(HOMESERVER_IDS[3].to_string())
        .await
        .map_err(|e| anyhow!(e))?;
    match timeout(Duration::from_secs(1), processor.run(shutdown_rx.clone())).await {
        Ok(_) => panic!("Event processor should timeout"),
        Err(_) => {} // expected timeout
    }

    Ok(())
}

fn create_mock_event_processors() -> HashMap<String, MockEventProcessor> {
    let mut event_processor_hashmap: HashMap<String, MockEventProcessor> = HashMap::new();
    let success = MockEventProcessor {
        homeserver_id: HOMESERVER_IDS[0].to_string(),
        timeout: None,
        processor_status: ProcessResult::Success("Success finished!".to_string()),
    };
    event_processor_hashmap.insert(HOMESERVER_IDS[0].to_string(), success);

    let error = MockEventProcessor {
        homeserver_id: HOMESERVER_IDS[1].to_string(),
        timeout: None,
        processor_status: ProcessResult::Error("Event processor error!".to_string().into()),
    };
    event_processor_hashmap.insert(HOMESERVER_IDS[1].to_string(), error);

    let error = MockEventProcessor {
        homeserver_id: HOMESERVER_IDS[2].to_string(),
        timeout: None,
        processor_status: ProcessResult::Panic(),
    };
    event_processor_hashmap.insert(HOMESERVER_IDS[2].to_string(), error);

    let panic = MockEventProcessor {
        homeserver_id: HOMESERVER_IDS[3].to_string(),
        timeout: Some(Duration::from_secs(2)),
        processor_status: ProcessResult::Success("Success finished!".to_string()),
    };
    event_processor_hashmap.insert(HOMESERVER_IDS[3].to_string(), panic);

    event_processor_hashmap
}
