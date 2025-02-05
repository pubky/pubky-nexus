use log::error;
use log::info;
use pubky_nexus::types::DynError;
use pubky_nexus::PubkyConnector;
use pubky_nexus::{Config, EventProcessor, StackManager};
use tokio::time::{sleep, Duration};

/// Watches over a homeserver `/events` and writes into the Nexus databases
#[tokio::main]
async fn main() -> Result<(), DynError> {
    let config = Config::from_env();

    StackManager::setup(&config).await;

    PubkyConnector::initialise(&config, None).await?;

    // Create and configure the event processor
    let mut event_processor = EventProcessor::from_config(&config).await?;
    event_processor.start().await
}
