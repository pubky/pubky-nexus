use log::info;
use pubky_nexus::{setup, Config, EventProcessor};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env();
    setup(&config).await;

    let mut event_processor = EventProcessor::new(&config).await;

    loop {
        info!("Fetching events...");
        event_processor.run().await?;
        sleep(Duration::from_secs(5)).await; // Wait for 5 seconds before fetching events again
    }
}
