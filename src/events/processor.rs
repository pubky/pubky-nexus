use std::error::Error;

use super::error::EventProcessorError;
use super::Event;
use crate::events::retry::event::RetryEvent;
use crate::types::DynError;
use crate::PubkyConnector;
use crate::{models::homeserver::Homeserver, Config};
use opentelemetry::trace::{FutureExt, TraceContextExt, Tracer};
use opentelemetry::{global, Context};
use pubky_app_specs::PubkyId;
use tracing::{debug, error, info};

pub struct EventProcessor {
    pub homeserver: Homeserver,
    limit: u32,
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
        let homeserver = Homeserver::new(id).await.unwrap();
        Self {
            homeserver,
            limit: 1000,
        }
    }

    pub async fn from_config(config: &Config) -> Result<Self, DynError> {
        let homeserver = Homeserver::from_config(config).await?;
        let limit = config.events_limit;

        info!(
            "Initialized Event Processor for homeserver: {:?}",
            homeserver
        );

        Ok(Self { homeserver, limit })
    }

    pub async fn run(&mut self) -> Result<(), DynError> {
        let lines = {
            let tracer = global::tracer("nexus.watcher");
            let span = tracer.start("Polling Events");
            let cx = Context::new().with_span(span);
            self.poll_events()
                .with_context(cx)
                .await
                .unwrap_or_default()
        };
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

        let response_text = {
            let pubky_client = PubkyConnector::get_pubky_client()?;
            let url = format!(
                "https://{}/events/?cursor={}&limit={}",
                self.homeserver.id, self.homeserver.cursor, self.limit
            );

            let response = pubky_client.get(url).send().await.map_err(|e| {
                Box::new(EventProcessorError::PubkyClientError {
                    message: format!("{:?}", e.source()),
                })
            })?;

            response.text().await?
        };

        let lines: Vec<String> = response_text.trim().lines().map(String::from).collect();
        debug!("Homeserver response lines {:?}", lines);

        if lines.is_empty() || (lines.len() == 1 && lines[0].is_empty()) {
            info!("No new events");
            return Ok(None);
        }

        Ok(Some(lines))
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
                    let tracer = global::tracer("nexus.watcher");
                    let span = tracer.start("Event Line");
                    let cx = Context::new().with_span(span);
                    debug!("Processing event: {:?}", event);
                    self.handle_event(event).with_context(cx).await?;
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
