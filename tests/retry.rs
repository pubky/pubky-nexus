use anyhow::Result;
use chrono::Utc;
use pkarr::mainline::Testnet;
use pubky::PubkyClient;
use pubky_nexus::{
    db::kv::index::sorted_sets::SortOrder,
    events::{
        Event, EventFailed, EventInfo, EventType, ResourceType, EVENT_ERROR_PREFIX,
        EVENT_RECOVERED_PREFIX,
    },
    setup,
    types::PubkyId,
    Config, RedisOps,
};

struct RetryTest {
    client: PubkyClient,
    config: Config,
}

impl RetryTest {
    async fn setup() -> Result<Self> {
        let config = Config::from_env();
        setup(&config).await;
        let testnet = Testnet::new(10);
        let client = PubkyClient::test(&testnet);
        Ok(Self { client, config })
    }

    async fn create_failed_event(
        &self,
        uri: &str,
        pubky: &str,
        event_id: &str,
        attempts: i32,
    ) -> Result<EventInfo> {
        let event = Event::new(
            String::from(uri),
            EventType::Put,
            ResourceType::Post {
                author_id: PubkyId(String::from(pubky)),
                post_id: String::from(event_id),
            },
        );
        let event_info = EventInfo::new(event, Utc::now().timestamp_millis(), attempts, None);
        event_info.put_index_json(&[uri]).await.unwrap();
        EventFailed::log(&event_info).await.unwrap();
        Ok(event_info)
    }
}

#[tokio::test]
async fn test_basic_retry() -> Result<()> {
    let test = RetryTest::setup().await?;
    let uri = "pubky://test/pub/app/post/post_id_1";

    // Create a failed event
    let event_info = test.create_failed_event(uri, "test", "post_id", 0).await?;

    // Attempt retry
    event_info
        .retry(&test.client, 3)
        .await
        .expect("Testing Retry failed");

    // Verify event state
    let event = EventInfo::try_from_index_json(&[uri]).await;
    assert!(event.unwrap().is_none(), "Event should no longer exist");

    let recovered_event = EventInfo::try_from_index_json(&[EVENT_RECOVERED_PREFIX, uri]).await;
    assert!(
        recovered_event.unwrap().is_some(),
        "Event should exist in the recovered state"
    );

    let failed_events = EventFailed::try_from_index_sorted_set(
        &[EventFailed::prefix().await.as_str()],
        None,
        None,
        None,
        None,
        SortOrder::Ascending,
    )
    .await;
    assert!(
        !failed_events
            .unwrap()
            .unwrap()
            .iter()
            .any(|(item, _)| item == uri),
        "Event should not exist in the failed state"
    );

    Ok(())
}

#[tokio::test]
async fn test_max_retry_attempts() -> Result<()> {
    let test = RetryTest::setup().await?;
    let uri = "pubky://test/pub/app/post/post_id_2";

    // Create failed event
    let event_info = test
        .create_failed_event(uri, "test", "post_id_2", test.config.max_retries - 1)
        .await?;

    // Retry until max attempts

    event_info
        .retry(&test.client, test.config.max_retries)
        .await
        .expect("Testing retry failed");

    // Verify event state
    let event = EventInfo::try_from_index_json(&[uri]).await;
    assert!(event.unwrap().is_none(), "Event should no longer exist");

    let recovered_event = EventInfo::try_from_index_json(&[EVENT_RECOVERED_PREFIX, uri]).await;
    assert!(
        recovered_event.unwrap().is_none(),
        "Event should not exist in the recovered state"
    );

    let error_event = EventInfo::try_from_index_json(&[EVENT_ERROR_PREFIX, uri]).await;
    assert!(
        error_event.unwrap().is_some(),
        "Event should exist in the error state"
    );

    let failed_events = EventFailed::try_from_index_sorted_set(
        &[EventFailed::prefix().await.as_str()],
        None,
        None,
        None,
        None,
        SortOrder::Ascending,
    )
    .await;
    assert!(
        !failed_events
            .unwrap()
            .unwrap()
            .iter()
            .any(|(item, _)| item == uri),
        "Event should not exist in the failed state"
    );
    Ok(())
}
