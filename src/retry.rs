use chrono::Utc;
use log::{error, info};
use pkarr::mainline::Testnet;
use pubky::PubkyClient;
use pubky_nexus::events::{EventFailed, EventInfo};
use pubky_nexus::RedisOps;
use pubky_nexus::{db::kv::index::sorted_sets::SortOrder, setup, Config};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::time::{interval, Duration};

fn init_pubky_client(config: &Config) -> PubkyClient {
    if config.testnet {
        let testnet = Testnet {
            bootstrap: vec![config.bootstrap.clone()],
            nodes: vec![],
        };
        PubkyClient::test(&testnet)
    } else {
        PubkyClient::default()
    }
}

const MAX_CONCURRENT_RETRIES: usize = 10;
const RETRY_INTERVAL: u64 = 5;

#[tokio::main]
async fn main() {
    let config = Config::from_env();
    setup(&config).await;
    let pubky_client = init_pubky_client(&config);

    info!("Starting retry cron job.");

    let retry_interval = Duration::from_secs(RETRY_INTERVAL);
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_RETRIES));
    let mut interval = interval(retry_interval);

    loop {
        interval.tick().await;
        let now = Utc::now().timestamp_millis();

        match EventFailed::list(
            Some((now as f64) - ((RETRY_INTERVAL as f64) * 1000.0)),
            Some(now as f64),
            None,
            None,
            SortOrder::Ascending,
        )
        .await
        {
            Ok(Some(failed_events)) => {
                for (event_uri, _) in failed_events {
                    let permit = semaphore.clone().acquire_owned().await.unwrap();
                    let event_uri = event_uri.clone();
                    let client = pubky_client.clone();

                    tokio::spawn(async move {
                        let _permit = permit; // Keep the permit until the task is done
                        match EventInfo::try_from_index_json(&[event_uri.as_str()]).await {
                            Ok(Some(event_info)) => {
                                event_info.retry(&client, config.max_retries).await
                            }
                            Ok(None) => {
                                error!("Failed event's info not found: {}", event_uri);
                                Ok(())
                            }
                            Err(e) => {
                                error!("Error getting event info for: {}: {}", event_uri, e);
                                Ok(())
                            }
                        }
                    });
                }
            }
            Ok(None) => info!("No failed events found."),
            Err(e) => error!("Error fetching failed events: {}", e),
        }
    }
}
