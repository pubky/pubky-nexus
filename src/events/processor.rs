use std::sync::Arc;
use std::time::Duration;

use super::error::EventProcessorError;
use super::Event;
use crate::events::retry::event::RetryEvent;
use crate::types::DynError;
use crate::PubkyConnector;
use crate::{models::homeserver::Homeserver, Config};
use log::{debug, error, info};
use pubky_app_specs::PubkyId;
use tokio::sync::Semaphore;
use tokio::time::sleep;

#[derive(Clone)]
pub struct EventProcessor {
    limit: u32,
    sleep: u64,
    max_processors: usize,
}

impl EventProcessor {
    /// Creates a new `EventProcessor` instance for testing purposes.
    ///
    /// This function initializes an `EventProcessor` configured with:
    /// - A mock homeserver constructed using the provided `homeserver_url` and `homeserver_pubky`.
    /// - A default configuration, including an HTTP client, a limit of 1000 events, and a sender channel.
    ///
    /// It is designed for use in integration tests, benchmarking scenarios, or other test environments
    /// where a controlled and predictable `EventProcessor` instance is required.
    ///
    /// # Parameters
    /// - `homeserver_id`: A `String` representing the URL of the homeserver to be used in the test environment.
    /// - `tx`: A `RetryManagerSenderChannel` used to handle outgoing messages or events.
    pub async fn test(homeserver_id: String) -> Self {
        let id = PubkyId::try_from(&homeserver_id).expect("Homeserver ID should be valid");
        let homeserver = Homeserver::new(id);
        homeserver.save().await.unwrap();
        Self {
            limit: 1000,
            sleep: 100,
            max_processors: 1,
        }
    }

    pub async fn from_config(config: &Config) -> Result<Self, DynError> {
        Homeserver::from_config(config).await?;
        let limit = config.events_limit;
        let sleep = config.watcher_sleep;
        let max_processors = config.max_processors;

        info!(
            "Initialized Event Processor for with limit: {}, sleep: {}, max_processors: {}",
            limit, sleep, max_processors
        );

        Ok(Self {
            limit,
            sleep,
            max_processors,
        })
    }

    pub async fn start(&mut self) -> Result<(), DynError> {
        // Create a semaphore to limit the number of concurrent processors
        let processor_semaphore = Arc::new(Semaphore::new(self.max_processors));

        loop {
            info!("Fetching events...");
            let homeservers = Homeserver::get_next_homeservers(
                processor_semaphore.available_permits() as i8,
                self.sleep,
            )
            .await?;
            for mut homeserver in homeservers {
                let permit = processor_semaphore.clone().acquire_owned().await?;
                homeserver.last_polled_at = chrono::Utc::now().timestamp_millis();
                homeserver.save().await?;
                let mut processor = self.clone();
                tokio::spawn(async move {
                    if let Err(e) = processor.run(homeserver).await {
                        error!("Uncaught error occurred while processing events: {:?}", e);
                    }
                    drop(permit);
                });
            }
            // Wait for X milliseconds before fetching events again
            sleep(Duration::from_millis(self.sleep)).await;
        }
    }

    pub async fn run(&mut self, homeserver: Homeserver) -> Result<(), DynError> {
        let lines = {
            self.poll_events(homeserver.clone())
                .await
                .unwrap_or_default()
        };
        if let Some(lines) = lines {
            self.process_event_lines(homeserver, lines).await?;
        };
        Ok(())
    }

    /// Polls new events from the homeserver.
    ///
    /// It sends a GET request to the homeserver's events endpoint
    /// using the current cursor and a specified limit. It retrieves new event
    /// URIs in a newline-separated format, processes it into a vector of strings,
    /// and returns the result.
    async fn poll_events(
        &mut self,
        homeserver: Homeserver,
    ) -> Result<Option<Vec<String>>, DynError> {
        debug!("Polling new events from homeserver");

        let response: String;
        {
            let pubky_client = PubkyConnector::get_pubky_client()?;
            response = pubky_client
                .get(format!(
                    "https://{}/events/?cursor={}&limit={}",
                    homeserver.id, homeserver.cursor, self.limit
                ))
                .send()
                .await?
                .text()
                .await?;
        }

        let lines: Vec<String> = response.trim().split('\n').map(|s| s.to_string()).collect();
        debug!("Homeserver response lines {:?}", lines);

        if lines.len() == 1 && lines[0].is_empty() {
            info!("No new events");
            Ok(None)
        } else {
            Ok(Some(lines))
        }
    }

    /// Processes a batch of event lines retrieved from the homeserver.
    ///
    /// This function iterates over a vector of event URIs, handling each line based on its content:
    /// - Lines starting with `cursor:` update the cursor for the homeserver and save it to the index.
    /// - Other lines are parsed into events and processed accordingly. If parsing fails, an error is logged.
    ///
    /// # Parameters
    /// - `lines`: A vector of strings representing event lines retrieved from the homeserver.
    pub async fn process_event_lines(
        &mut self,
        mut homeserver: Homeserver,
        lines: Vec<String>,
    ) -> Result<(), DynError> {
        for line in &lines {
            if line.starts_with("cursor:") {
                if let Some(cursor) = line.strip_prefix("cursor: ") {
                    homeserver.cursor = cursor.to_string();
                    homeserver.save().await?;
                    info!("Cursor for the next request: {}", cursor);
                }
            } else {
                let event = match Event::parse_event(line) {
                    Ok(event) => event,
                    Err(e) => {
                        error!("{}", e);
                        None
                    }
                };
                if let Some(event) = event {
                    debug!("Processing event: {:?}", event);
                    self.handle_event(event).await?;
                }
            }
        }

        Ok(())
    }

    /// Processes an event and track the fail event it if necessary
    /// # Parameters:
    /// - `event`: The event to be processed
    async fn handle_event(&mut self, event: Event) -> Result<(), DynError> {
        if let Err(e) = event.clone().handle().await {
            if let Some((index_key, retry_event)) = extract_retry_event_info(&event, e) {
                error!("{}, {}", retry_event.error_type, index_key);
                if let Err(err) = retry_event.put_to_index(index_key).await {
                    error!("Failed to put event to retry index: {}", err);
                }
            }
        }
        Ok(())
    }
}

/// Extracts retry-related information from an event and its associated error
///
/// # Parameters
/// - `event`: Reference to the event for which retry information is being extracted
/// - `error`: Determines whether the event is eligible for a retry or should be discarded
fn extract_retry_event_info(event: &Event, error: DynError) -> Option<(String, RetryEvent)> {
    let retry_event = match error.downcast_ref::<EventProcessorError>() {
        Some(EventProcessorError::InvalidEventLine { message }) => {
            error!("{}", message);
            return None;
        }
        Some(event_processor_error) => RetryEvent::new(event_processor_error.clone()),
        // Others errors must be logged at least for now
        None => {
            error!("Unhandled error type for URI: {}, {:?}", event.uri, error);
            return None;
        }
    };

    // Generate a compress index to save in the cache
    let index = match RetryEvent::generate_index_key(&event.uri) {
        Some(retry_index) => retry_index,
        None => {
            return None;
        }
    };
    Some((format!("{}:{}", event.event_type, index), retry_event))
}
