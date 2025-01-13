use log::error;
use log::info;
use pubky_nexus::events::retry::RetryManager;
use pubky_nexus::events::retry::SenderMessage;
use pubky_nexus::events::retry::CHANNEL_BUFFER;
use pubky_nexus::PubkyConnector;
use pubky_nexus::{setup, Config, EventProcessor};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

const RETRY_THRESHOLD: u8 = 5;

/// Watches over a homeserver `/events` and writes into the Nexus databases
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    let config = Config::from_env();
    // Initializes database connectors for Neo4j and Redis
    setup(&config).await;

    // Initializes the PubkyConnector with the configuration
    PubkyConnector::initialise(&config, None)?;

    // Initializes a retry manager and ensures robustness by managing retries asynchronously
    let retry_manager = RetryManager::initialise(mpsc::channel(CHANNEL_BUFFER));
    // Prepare the sender channel to send the messages to the retry manager
    let sender_clone = retry_manager.sender.clone();
    // Create new asynchronous task to control the failed events
    tokio::spawn(async move {
        let _ = retry_manager.exec().await;
    });

    // Create and configure the event processor
    let mut event_processor = EventProcessor::from_config(&config, sender_clone).await?;

    // Experimental. We need to think how/where achieve that
    // Maybe add in .env file...
    let mut retry_failed_events = 0;

    loop {
        info!("Fetching events...");
        if let Err(e) = event_processor.run().await {
            error!("Uncaught error occurred while processing events: {:?}", e);
        }
        // Wait for X milliseconds before fetching events again
        sleep(Duration::from_millis(config.watcher_sleep)).await;
        retry_failed_events += 1;

        if RETRY_THRESHOLD == retry_failed_events {
            let sender = event_processor.sender.lock().await;
            let _ = sender
                .send(SenderMessage::Retry(
                    event_processor.homeserver.id.to_string(),
                ))
                .await;
        }
    }
}
