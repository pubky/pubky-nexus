use log::info;
use pubky_nexus::{setup, Config, EventProcessor};
use tokio::time::{sleep, Duration};

/// Watches over a homeserver `/events` and writes into the Nexus databases
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    let config = Config::from_env();
    setup(&config).await;
    let mut event_processor = EventProcessor::from_config(&config).await;

    loop {
        info!("Fetching events...");
        event_processor.run().await?;
        // Wait for X milliseconds before fetching events again
        sleep(Duration::from_millis(config.watcher_sleep)).await;
    }
}
