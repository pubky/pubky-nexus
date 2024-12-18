use log::error;
use log::info;
use pubky_nexus::PubkyConnector;
use pubky_nexus::{setup, Config, EventProcessor};
use tokio::time::{sleep, Duration};

/// Watches over a homeserver `/events` and writes into the Nexus databases
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    let config = Config::from_env();
    setup(&config).await;
    PubkyConnector::initialise(&config, None)?;
    let mut event_processor = EventProcessor::from_config(&config).await?;

    loop {
        info!("Fetching events...");
        if let Err(e) = event_processor.run().await {
            error!("Uncaught error occurred while processing events: {:?}", e);
        }
        // Wait for X milliseconds before fetching events again
        sleep(Duration::from_millis(config.watcher_sleep)).await;
    }
}
