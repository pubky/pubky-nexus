use super::error::EventProcessorError;
use super::retry::SenderChannel;
use super::retry::SenderMessage;
use super::Event;
use crate::events::retry::RetryEvent;
use crate::types::DynError;
use crate::types::PubkyId;
use crate::{models::homeserver::Homeserver, Config};
use log::{debug, error, info};
use reqwest::Client;
use serde::Deserialize;
use serde::Serialize;

pub struct EventProcessor {
    http_client: Client,
    pub homeserver: Homeserver,
    limit: u32,
    pub sender: SenderChannel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EventErrorType {
    NotResolveHomeserver,
    PubkyClientError,
    MissingDependency,
    GraphError,
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
            http_client: Client::new(),
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
    /// - `homeserver_url`: A `String` representing the URL of the homeserver to be used in the test environment.
    /// - `homeserver_pubky`: A `PubkyId` instance representing the unique identifier for the homeserver's public key.
    /// - `tx`: A `SenderChannel` used to handle outgoing messages or events.
    pub async fn test(
        homeserver_url: String,
        homeserver_pubky: PubkyId,
        tx: SenderChannel,
    ) -> Self {
        let homeserver = Homeserver::new(homeserver_pubky, homeserver_url)
            .await
            .unwrap();
        Self {
            http_client: Client::new(),
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
        let res = self
            .http_client
            .get(format!(
                "{}/events/?cursor={}&limit={}",
                self.homeserver.url, self.homeserver.cursor, self.limit
            ))
            .send()
            .await?
            .text()
            .await?;

        let lines: Vec<String> = res.trim().split('\n').map(|s| s.to_string()).collect();
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
                        error!("Error while creating event line from line: {}", e);
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
                error!("Error while handling event: {}", e);

                let retry_event = match e.downcast_ref::<EventProcessorError>() {
                    Some(EventProcessorError::UserNotSync) => RetryEvent::new(
                        &event.uri,
                        &event.event_type,
                        None,
                        EventErrorType::GraphError,
                    ),
                    Some(EventProcessorError::MissingDependency { dependency }) => RetryEvent::new(
                        &event.uri,
                        &event.event_type,
                        Some(dependency.clone()),
                        EventErrorType::MissingDependency,
                    ),
                    // Other retry errors must be ignored
                    _ => return Ok(()),
                };

                let sender = self.sender.lock().await;
                match sender
                    .send(SenderMessage::Add(self.homeserver.id.clone(), retry_event))
                    .await
                {
                    Ok(_) => {
                        info!("Message send succesfully from the channel");
                        // TODO: Investigate non-blocking alternatives
                        // The current use of `tokio::time::sleep` is intended to handle a situation where tasks in other threads
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
                    Err(e) => error!("Err, {:?}", e),
                }
                Ok(())
            }
        }
    }
}
