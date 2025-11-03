use crate::events::errors::EventProcessorError;
use crate::events::handlers;
use crate::events::moderation::Moderation;
use crate::events::retry::event::RetryEvent;
use crate::service::traits::TEventProcessor;
use nexus_common::db::PubkyClient;
use nexus_common::models::event::{Event, EventType};
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use opentelemetry::trace::{FutureExt, Span, TraceContextExt, Tracer};
use opentelemetry::{global, Context, KeyValue};
use pubky_app_specs::PubkyId;
use pubky_app_specs::{PubkyAppObject, Resource};
use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch::Receiver;
use tracing::{debug, error, info};

pub struct EventProcessor {
    pub homeserver: Homeserver,
    pub limit: u32,
    pub files_path: PathBuf,
    pub tracer_name: String,
    pub moderation: Arc<Moderation>,
    pub shutdown_rx: Receiver<bool>,
}

#[async_trait::async_trait]
impl TEventProcessor for EventProcessor {
    fn get_homeserver_id(&self) -> PubkyId {
        self.homeserver.id.clone()
    }

    async fn run_internal(self: Arc<Self>) -> Result<(), DynError> {
        let maybe_event_lines = {
            let tracer = global::tracer(self.tracer_name.clone());
            let span = tracer.start("Polling Events");
            let cx = Context::new().with_span(span);
            self.poll_events()
                .with_context(cx)
                .await
                .inspect_err(|e| error!("Error polling events: {e:?}"))?
        };

        match maybe_event_lines {
            None => info!("No new events"),
            Some(event_lines) => {
                info!("Processing {} event lines", event_lines.len());
                self.process_event_lines(event_lines).await?;
            }
        }

        Ok(())
    }
}

impl EventProcessor {
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
    pub async fn process_event_lines(&self, lines: Vec<String>) -> Result<(), DynError> {
        for line in &lines {
            let id = self.homeserver.id.clone();

            if *self.shutdown_rx.borrow() {
                debug!("Shutdown detected while processing HS {id}, exiting event processing loop");
                return Ok(());
            }

            if let Some(cursor) = line.strip_prefix("cursor: ") {
                Homeserver::from_cursor(id, cursor).put_to_index().await?;
                info!("Cursor for the next request: {cursor}");
            } else {
                debug!("New event: {}", line);
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
                    self.handle_event(&event).with_context(cx).await?;
                }
            }
        }

        Ok(())
    }

    /// Processes an event and track the fail event it if necessary
    /// # Parameters:
    /// - `event`: The event to be processed
    async fn handle_event(&self, event: &Event) -> Result<(), DynError> {
        if let Err(e) = self.handle(event).await {
            if let Some((index_key, retry_event)) = extract_retry_event_info(event, e) {
                error!("{}, {}", retry_event.error_type, index_key);
                if let Err(err) = retry_event.put_to_index(index_key).await {
                    error!("Failed to put event to retry index: {}", err);
                }
            }
        }
        Ok(())
    }

    pub async fn handle(&self, event: &Event) -> Result<(), DynError> {
        // TODO: (524): add method to insert event in to reddis sorted set
        // - reddis ordered set, so that can be ordered by cursor
        // - cursor is local timestamp in milliseconds encoded using crockford32
        // <cursor>: { <PUT|DEL>, <URL> }
        match event.event_type {
            EventType::Put => self.handle_put_event(event).await,
            EventType::Del => self.handle_del_event(event).await,
        }
    }

    /// Handles a PUT event by fetching the blob from the homeserver
    /// and using the importer to convert it to a PubkyAppObject.
    pub async fn handle_put_event(&self, event: &Event) -> Result<(), DynError> {
        debug!("Handling PUT event for URI: {}", event.uri);

        let response;
        {
            let pubky_client =
                PubkyClient::get().map_err(|e| EventProcessorError::PubkyClientError {
                    message: e.to_string(),
                })?;

            response = match pubky_client.get(&event.uri).send().await {
                Ok(response) => response,
                Err(e) => {
                    return Err(EventProcessorError::PubkyClientError {
                        message: format!("{e}"),
                    }
                    .into())
                }
            };
        } // drop the pubky_client lock

        let blob = response.bytes().await?;
        let resource = event.parsed_uri.resource.clone();

        // Use the new importer from pubky-app-specs
        let pubky_object = PubkyAppObject::from_resource(&resource, &blob).map_err(|e| {
            EventProcessorError::PubkyClientError {
                message: format!(
                    "The importer could not create PubkyAppObject from Uri and Blob: {e}"
                ),
            }
        })?;

        let user_id = event.parsed_uri.user_id.clone();
        match (pubky_object, resource) {
            (PubkyAppObject::User(user), Resource::User) => {
                handlers::user::sync_put(user, user_id).await?
            }
            (PubkyAppObject::Post(post), Resource::Post(post_id)) => {
                handlers::post::sync_put(post, user_id, post_id).await?
            }
            (PubkyAppObject::Follow(_follow), Resource::Follow(followee_id)) => {
                handlers::follow::sync_put(user_id, followee_id).await?
            }
            (PubkyAppObject::Mute(_mute), Resource::Mute(muted_id)) => {
                handlers::mute::sync_put(user_id, muted_id).await?
            }
            (PubkyAppObject::Bookmark(bookmark), Resource::Bookmark(bookmark_id)) => {
                handlers::bookmark::sync_put(user_id, bookmark, bookmark_id).await?
            }
            (PubkyAppObject::Tag(tag), Resource::Tag(tag_id)) => {
                if self.moderation.should_delete(&tag, user_id.clone()).await {
                    Moderation::apply_moderation(tag, event.files_path.clone()).await?
                } else {
                    handlers::tag::sync_put(tag, user_id, tag_id).await?
                }
            }
            (PubkyAppObject::File(file), Resource::File(file_id)) => {
                handlers::file::sync_put(
                    file,
                    event.uri.clone(),
                    user_id,
                    file_id,
                    self.files_path.clone(),
                )
                .await?
            }
            other => {
                debug!("Event type not handled, Resource: {:?}", other);
            }
        }
        Ok(())
    }

    pub async fn handle_del_event(&self, event: &Event) -> Result<(), DynError> {
        debug!("Handling DEL event for URI: {}", event.uri);

        let user_id = event.parsed_uri.user_id.clone();
        match event.parsed_uri.resource.clone() {
            Resource::User => handlers::user::del(user_id).await?,
            Resource::Post(post_id) => handlers::post::del(user_id, post_id).await?,
            Resource::Follow(followee_id) => handlers::follow::del(user_id, followee_id).await?,
            Resource::Mute(muted_id) => handlers::mute::del(user_id, muted_id).await?,
            Resource::Bookmark(bookmark_id) => {
                handlers::bookmark::del(user_id, bookmark_id).await?
            }
            Resource::Tag(tag_id) => handlers::tag::del(user_id, tag_id).await?,
            Resource::File(file_id) => {
                handlers::file::del(&user_id, file_id, event.files_path.clone()).await?
            }
            other => {
                debug!("DEL event type not handled for resource: {:?}", other);
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
