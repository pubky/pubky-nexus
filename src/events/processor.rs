use super::error::EventProcessorError;
use super::retry::manager::{SenderChannel, SenderMessage};
use super::Event;
use crate::events::retry::event::RetryEvent;
use crate::types::DynError;
use crate::types::PubkyId;
use crate::PubkyConnector;
use crate::{models::homeserver::Homeserver, Config};
use log::{debug, error, info};

pub struct EventProcessor {
    pub homeserver: Homeserver,
    limit: u32,
    pub sender: SenderChannel,
}

impl EventProcessor {
    pub async fn from_config(config: &Config, tx: SenderChannel) -> Result<Self, DynError> {
        let homeserver = Homeserver::from_config(config).await?;
        let limit = config.events_limit;

        info!(
            "Initialized Event Processor for homeserver: {:?}",
            homeserver
        );

        Ok(Self {
            homeserver,
            limit,
            sender: tx,
        })
    }

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
    /// - `tx`: A `SenderChannel` used to handle outgoing messages or events.
    pub async fn test(homeserver_id: String, tx: SenderChannel) -> Self {
        let id = PubkyId(homeserver_id.to_string());
        let homeserver = Homeserver::new(id).await.unwrap();
        Self {
            homeserver,
            limit: 1000,
            sender: tx,
        }
    }

    pub async fn run(&mut self) -> Result<(), DynError> {
        let lines = { self.poll_events().await.unwrap_or_default() };
        if let Some(lines) = lines {
            self.process_event_lines(lines).await?;
        };
        Ok(())
    }

    /// Polls new events from the homeserver.
    ///
    /// It sends a GET request to the homeserver's events endpoint
    /// using the current cursor and a specified limit. It retrieves new event
    /// URIs in a newline-separated format, processes it into a vector of strings,
    /// and returns the result.
    async fn poll_events(&mut self) -> Result<Option<Vec<String>>, DynError> {
        debug!("Polling new events from homeserver");

        let response: String;
        {
            let pubky_client = PubkyConnector::get_pubky_client()?;
            response = pubky_client
                .get(format!(
                    "https://{}/events/?cursor={}&limit={}",
                    self.homeserver.id, self.homeserver.cursor, self.limit
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
    pub async fn process_event_lines(&mut self, lines: Vec<String>) -> Result<(), DynError> {
        for line in &lines {
            if line.starts_with("cursor:") {
                if let Some(cursor) = line.strip_prefix("cursor: ") {
                    self.homeserver.cursor = cursor.to_string();
                    self.homeserver.put_to_index().await?;
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

    // Generic retry on event handler
    async fn handle_event(&self, event: Event) -> Result<(), DynError> {
        match event.clone().handle().await {
            Ok(_) => Ok(()),
            Err(e) => {
                let retry_event = match e.downcast_ref::<EventProcessorError>() {
                    Some(EventProcessorError::InvalidEventLine { message }) => {
                        error!("{}", message);
                        return Ok(());
                    }
                    Some(event_processor_error) => RetryEvent::new(event_processor_error.clone()),
                    // Others errors must be logged at least for now
                    None => {
                        error!("Unhandled error type for URI: {}, {:?}", event.uri, e);
                        return Ok(());
                    }
                };

                // Generate a compress index to save in the cache
                let index = match RetryEvent::generate_index_key(&event.uri) {
                    Some(retry_index) => retry_index,
                    None => {
                        // Unlikely to be reached, as it would typically fail during the validation process
                        return Ok(());
                    }
                };
                let index_key = format!("{}:{}", event.event_type, index);

                // Send event to the retry manager
                let sender = self.sender.lock().await;
                match sender
                    .send(SenderMessage::ProcessEvent(index_key, retry_event))
                    .await
                {
                    Ok(_) => {
                        // TODO: Investigate non-blocking alternatives
                        // The current use of `tokio::time::sleep` (in the watcher tests) is intended to handle a situation where tasks in other threads
                        // are not being tracked. This could potentially lead to issues with writing `RetryEvents` in certain cases.
                        // Considerations:
                        // - Determine if this delay is genuinely necessary. Testing may reveal that the `RetryManager` thread
                        //   handles retries adequately while the watcher remains active, making this timer redundant.
                        // - If the delay is required, explore a more efficient solution that does not block the thread
                        //   and ensures proper handling of tasks in other threads.
                        //
                        // For now, the sleep is deactivated
                        //tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                    Err(e) => error!("Failed to send message to RetryManager: {:?}", e),
                }
                Ok(())
            }
        }
    }
}
