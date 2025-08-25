use super::moderation::Moderation;
use super::Event;
use crate::events::errors::EventProcessorError;
use crate::events::retry::event::RetryEvent;
use nexus_common::db::PubkyClient;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use nexus_common::{get_files_dir_test_pathbuf, WatcherConfig};
use opentelemetry::trace::{FutureExt, Span, TraceContextExt, Tracer};
use opentelemetry::{global, Context, KeyValue};
use pubky_app_specs::PubkyId;
use std::error::Error;
use std::path::PathBuf;
use tokio::sync::watch::Receiver;
use tracing::{debug, error, info};

/// This implements the creation logic for [`EventProcessor`] objects
pub struct EventProcessorFactory {
    pub limit: u32,
    pub files_path: PathBuf,
    pub tracer_name: String,
    pub moderation: Moderation,
}

impl EventProcessorFactory {
    /// Creates a new factory instance from the provided configuration
    pub fn from_config(config: &WatcherConfig) -> Self {
        Self {
            limit: config.events_limit,
            files_path: config.stack.files_path.clone(),
            tracer_name: config.name.clone(),
            moderation: Moderation {
                id: config.moderation_id.clone(),
                tags: config.moderated_tags.clone(),
            },
        }
    }

    /// Builds and returns a configured [`EventProcessor`] instance.
    /// # Arguments
    /// - `homeserver_id`: The ID of the homeserver to process
    pub async fn build(&self, homeserver_id: &str) -> Result<EventProcessor, DynError> {
        let homeserver_id = PubkyId::try_from(homeserver_id).map_err(DynError::from)?;
        let homeserver = Homeserver::get_by_id(homeserver_id)
            .await?
            .ok_or("Homeserver not found")?;

        Ok(EventProcessor {
            homeserver,
            limit: self.limit,
            files_path: self.files_path.clone(),
            tracer_name: self.tracer_name.clone(),
            moderation: self.moderation.clone(),
        })
    }
}

pub struct EventProcessor {
    pub homeserver: Homeserver,
    limit: u32,
    pub files_path: PathBuf,
    // TODO: Maybe we could define a name for each homeserver? Not sure about that.
    pub tracer_name: String,
    pub moderation: Moderation,
}

impl EventProcessor {
    /// Creates a new [`EventProcessor`] instance for testing purposes.
    ///
    /// This function initializes an [`EventProcessor`] configured with:
    /// - A mock homeserver constructed using the provided `homeserver_url` and `homeserver_pubky`.
    /// - A default configuration, including an HTTP client, a limit of 1000 events, and a sender channel.
    ///
    /// It is designed for use in integration tests, benchmarking scenarios, or other test environments
    /// where a controlled and predictable [`EventProcessor`] instance is required.
    ///
    /// # Parameters
    /// - `homeserver_id`: A `String` representing the URL of the homeserver to be used in the test environment.
    /// - `tx`: A `RetryManagerSenderChannel` used to handle outgoing messages or events.
    pub async fn test(homeserver_id: String) -> Self {
        let id = PubkyId::try_from(&homeserver_id).expect("Homeserver ID should be valid");
        let homeserver = Homeserver::new(id.clone());

        // hardcoded nexus-watcher/tests/utils/moderator_key.pkarr public key used by the moderator user on tests
        let moderation = Moderation {
            id: PubkyId::try_from("uo7jgkykft4885n8cruizwy6khw71mnu5pq3ay9i8pw1ymcn85ko")
                .expect("Hardcoded test moderation key should be valid"),
            tags: Vec::from(["label_to_moderate".to_string()]),
        };

        info!(
            "Watcher static files PATH during tests are stored inside of the watcher crate: {:?}",
            get_files_dir_test_pathbuf()
        );
        Self {
            homeserver,
            limit: 1000,
            files_path: get_files_dir_test_pathbuf(),
            tracer_name: String::from("watcher.test"),
            moderation,
        }
    }

    /// Runs the event processor. Polls new events from the homeserver and processes them
    /// # Parameters
    /// - `shutdown_rx`: A `Receiver<bool>` used to listen for shutdown signals
    pub async fn run(&self, shutdown_rx: Receiver<bool>) -> Result<String, DynError> {
        let lines = {
            let tracer = global::tracer(self.tracer_name.clone());
            let span = tracer.start("Polling Events");
            let cx = Context::new().with_span(span);
            self.poll_events().with_context(cx).await
        };

        match lines {
            Err(e) => {
                error!("Error polling events: {:?}", e);
                Err(e)
            }
            Ok(None) => {
                info!("No new events");
                Ok(String::new())
            }
            Ok(Some(lines)) => {
                info!("Processing {} event lines", lines.len());
                self.process_event_lines(shutdown_rx, lines).await
            }
        }
    }

    /// Polls new events from the homeserver.
    ///
    /// It sends a GET request to the homeserver's events endpoint
    /// using the current cursor and a specified limit. It retrieves new event
    /// URIs in a newline-separated format, processes it into a vector of strings,
    /// and returns the result.
    async fn poll_events(&self) -> Result<Option<Vec<String>>, DynError> {
        debug!("Polling new events from homeserver");

        let response_text = {
            let pubky_client =
                PubkyClient::get().map_err(|e| EventProcessorError::PubkyClientError {
                    message: e.to_string(),
                })?;
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
    pub async fn process_event_lines(
        &self,
        shutdown_rx: Receiver<bool>,
        lines: Vec<String>,
    ) -> Result<String, DynError> {
        println!("Processing {:?} event lines", lines);
        let mut new_cursor = String::new();
        for line in &lines {
            if *shutdown_rx.borrow() {
                info!("Shutdown detected, exiting event processing loop");
                return Ok(new_cursor);
            }

            // Cursor is the last line of the event list
            if line.starts_with("cursor:") {
                if let Some(cursor) = line.strip_prefix("cursor: ") {
                    new_cursor = String::from(cursor);
                    // TODO: Might be good idea to update all the homeserver cursors in a batch
                    self.homeserver
                        .persist_cursor(cursor.to_string())
                        .put_to_index()
                        .await?;
                }
            } else {
                let event = match Event::parse_event(line, self.files_path.clone()) {
                    Ok(event) => event,
                    Err(e) => {
                        error!("{}", e);
                        None
                    }
                };
                if let Some(event) = event {
                    let tracer = global::tracer(self.tracer_name.clone());
                    let mut span = tracer.start(event.parsed_uri.resource.to_string());
                    span.set_attribute(KeyValue::new("event.uri", event.uri.clone()));
                    span.set_attribute(KeyValue::new("event.type", event.event_type.to_string()));
                    span.set_attribute(KeyValue::new(
                        "event.user_id",
                        event.parsed_uri.user_id.to_string(),
                    ));
                    span.set_attribute(KeyValue::new(
                        "event.resource_id",
                        event.parsed_uri.resource.id().unwrap_or("".to_string()),
                    ));
                    let cx = Context::new().with_span(span);
                    debug!("Processing event: {:?}", event);
                    self.handle_event(event).with_context(cx).await?;
                }
            }
        }

        Ok(new_cursor)
    }

    /// Processes an event and track the fail event it if necessary
    /// # Parameters:
    /// - `event`: The event to be processed
    async fn handle_event(&self, event: Event) -> Result<(), DynError> {
        if let Err(e) = event.clone().handle(&self.moderation).await {
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
