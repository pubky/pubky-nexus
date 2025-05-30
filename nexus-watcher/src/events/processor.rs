use super::moderation::Moderation;
use super::Event;
use crate::events::errors::EventProcessorError;
use crate::events::retry::event::RetryEvent;
use nexus_common::db::PubkyClient;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use nexus_common::{WatcherConfig, FILES_DIR_TEST};
use opentelemetry::trace::{FutureExt, Span, TraceContextExt, Tracer};
use opentelemetry::{global, Context, KeyValue};
use pubky_app_specs::PubkyId;
use std::error::Error;
use std::path::PathBuf;
use tracing::{debug, error, info};

pub struct EventProcessor {
    pub homeserver: Homeserver,
    limit: u32,
    pub files_path: PathBuf,
    pub tracer_name: String,
    pub moderation: Moderation,
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

        // hardcoded nexus-watcher/tests/utils/moderator_key.pkarr public key used by the moderator user on tests
        let moderation = Moderation {
            id: PubkyId::try_from("uo7jgkykft4885n8cruizwy6khw71mnu5pq3ay9i8pw1ymcn85ko")
                .expect("Hardcoded test moderation key should be valid"),
            tags: Vec::from(["label_to_moderate".to_string()]),
        };

        info!(
            "Watcher static files PATH during tests are stored inside of the watcher crate: {:?}",
            PathBuf::from(FILES_DIR_TEST)
        );
        Self {
            homeserver,
            limit: 1000,
            files_path: PathBuf::from(FILES_DIR_TEST),
            tracer_name: String::from("watcher.test"),
            moderation,
        }
    }

    pub async fn from_config(config: &WatcherConfig) -> Result<Self, DynError> {
        let homeserver = Homeserver::from_config(config.homeserver.clone()).await?;
        let limit = config.events_limit;
        let files_path = config.stack.files_path.clone();
        let tracer_name = config.name.clone();

        let moderation = Moderation {
            id: config.moderation_id.clone(),
            tags: config.moderated_tags.clone(),
        };

        info!(
            "Initialized Event Processor for homeserver: {:?}",
            homeserver
        );

        Ok(Self {
            homeserver,
            limit,
            files_path,
            tracer_name,
            moderation,
        })
    }

    pub async fn run(&mut self) -> Result<(), DynError> {
        let lines = {
            let tracer = global::tracer(self.tracer_name.clone());
            let span = tracer.start("Polling Events");
            let cx = Context::new().with_span(span);
            self.poll_events().with_context(cx).await
        };

        match lines {
            Err(e) => {
                error!("Error polling events: {:?}", e);
                return Err(e);
            }
            Ok(None) => {
                info!("No new events");
            }
            Ok(Some(lines)) => {
                self.process_event_lines(lines).await?;
            }
        }

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
    pub async fn process_event_lines(&mut self, lines: Vec<String>) -> Result<(), DynError> {
        for line in &lines {
            if line.starts_with("cursor:") {
                if let Some(cursor) = line.strip_prefix("cursor: ") {
                    self.homeserver.cursor = cursor.to_string();
                    self.homeserver.put_to_index().await?;
                    info!("Cursor for the next request: {}", cursor);
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

        Ok(())
    }

    /// Processes an event and track the fail event it if necessary
    /// # Parameters:
    /// - `event`: The event to be processed
    async fn handle_event(&mut self, event: Event) -> Result<(), DynError> {
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
