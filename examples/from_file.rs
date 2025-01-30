use anyhow::Result;
use pubky_nexus::events::retry::manager::RetryManager;
use pubky_nexus::{setup, types::DynError, Config, EventProcessor};
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use std::sync::Arc;

// Create that file and add the file with that format
// PUT homeserver_uri
// DEL homeserver_uri
const FILE_PATH: &str = "examples/events.txt";

#[tokio::main]
async fn main() -> Result<(), DynError> {
    let config = Config::from_env();
    setup(&config).await;

    // Initializes a retry manager and ensures robustness by managing retries asynchronously
    let (receiver_channel, sender_channel) = RetryManager::init_channels();

    // Create new asynchronous task to control the failed events
    RetryManager::process_messages(receiver_channel).await;

    // Prepare the sender channel to send the messages to the retry manager
    let sender_clone = Arc::clone(&sender_channel);

    let mut event_processor = EventProcessor::from_config(&config, sender_clone).await?;

    let events = read_events_from_file().unwrap();
    event_processor.process_event_lines(events).await?;

    Ok(())
}

fn read_events_from_file() -> io::Result<Vec<String>> {
    let path = Path::new(FILE_PATH);
    let file = File::open(&path)?;

    let reader = io::BufReader::new(file);
    let lines = reader
        .lines()
        .filter_map(|line| line.ok()) // Filter out lines with errors
        .collect();

    Ok(lines)
}
