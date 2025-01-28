use log::error;
use log::info;
use pubky_nexus::events::retry::manager::RetryManager;
use pubky_nexus::PubkyConnector;
use pubky_nexus::{setup, Config, EventProcessor};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// Watches over a homeserver `/events` and writes into the Nexus databases
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    let config = Config::from_env();
    // Initializes database connectors for Neo4j and Redis
    setup(&config).await;

    // Initializes the PubkyConnector with the configuration
    PubkyConnector::initialise(&config, None).await?;

    // Initializes a retry manager and ensures robustness by managing retries asynchronously
    let (receiver_channel, sender_channel) = RetryManager::init_channels();

    // Create new asynchronous task to control the failed events
    RetryManager::process_messages(&receiver_channel).await;

    // Prepare the sender channel to send the messages to the retry manager
    let sender_clone = Arc::clone(&sender_channel);

    // Create and configure the event processor
    let mut event_processor = EventProcessor::from_config(&config, sender_clone).await?;

    loop {
        info!("Fetching events...");
        if let Err(e) = event_processor.run().await {
            error!("Uncaught error occurred while processing events: {:?}", e);
        }
        // Wait for X milliseconds before fetching events again
        sleep(Duration::from_millis(config.watcher_sleep)).await;
    }
}
