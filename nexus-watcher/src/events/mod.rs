use nexus_common::{db::PubkyConnector, models::user::UserIngestor};
pub mod event;

pub use event::{Event, EventType, ParseResult};

use crate::errors::EventProcessorError;
use nexus_common::universal_tag::homeserver_parsed_uri::HomeserverParsedUri;
use nexus_common::WatcherConfig;
use pubky_app_specs::{PubkyAppObject, Resource};
use std::sync::Arc;
use tracing::debug;

mod fetch;
pub mod handlers;
mod moderation;
pub mod retry;

pub(crate) use fetch::{
    fetch_capped, format_error_body, read_stream_capped, MAX_ERROR_BODY, MAX_EVENTS_BODY,
    MAX_RESOURCE_SIZE,
};
pub use moderation::Moderation;

/// Trait for handling events.
///
/// This trait abstracts event handling logic to allow for flexible implementations,
/// including mocked versions for testing.
#[async_trait::async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle(&self, event: &Event, max_file_size: u64) -> Result<(), EventProcessorError>;
}

/// Default implementation of `EventHandler` that uses the actual event handling logic.
pub struct DefaultEventHandler {
    moderation: Arc<Moderation>,
    ingestor: Arc<UserIngestor>,
}

impl DefaultEventHandler {
    pub fn new(moderation: Arc<Moderation>, ingestor: Arc<UserIngestor>) -> Self {
        Self {
            moderation,
            ingestor,
        }
    }

    /// Builds a handler, deriving its moderation rules and user ingestor from config.
    pub fn from_config(config: &WatcherConfig) -> Self {
        Self::new(
            Moderation::from_config(config),
            Arc::new(UserIngestor::from_config(&config.stack)),
        )
    }
}

#[async_trait::async_trait]
impl EventHandler for DefaultEventHandler {
    async fn handle(&self, event: &Event, max_file_size: u64) -> Result<(), EventProcessorError> {
        match event.event_type {
            EventType::Put => {
                handle_put_event(
                    event,
                    max_file_size,
                    self.moderation.clone(),
                    self.ingestor.clone(),
                )
                .await
            }
            EventType::Del => handle_del_event(event, self.ingestor.clone()).await,
        }?;

        event.to_event_line().store().await?;
        Ok(())
    }
}

pub async fn handle_put_event(
    event: &Event,
    max_file_size: u64,
    moderation: Arc<Moderation>,
    ingestor: Arc<UserIngestor>,
) -> Result<(), EventProcessorError> {
    debug!("Handling PUT event for URI: {}", event.uri);

    let pubky = PubkyConnector::get()?;
    let response = pubky.public_storage().get(&event.uri).await?;

    if !response.status().is_success() {
        let status = response.status();
        let (body, _exceeded) = read_stream_capped(response.bytes_stream(), MAX_ERROR_BODY)
            .await
            .unwrap_or_default();
        let body = format_error_body(&body, MAX_ERROR_BODY);

        let err_msg = format!(
            "Fetch resource failed {}: HTTP {status} - {body}",
            event.uri
        );
        return Err(EventProcessorError::client_error(err_msg))?;
    }

    let blob = fetch_capped(response, MAX_RESOURCE_SIZE as u64).await?;

    let resource = event.parsed_uri.resource().clone();

    // Use the new importer from pubky-app-specs.
    // `from_resource` runs spec validation; failures are deterministic and must
    // not be retried (a re-run produces the same error). Classify them as
    // `SpecValidation` so the retry queue stays clean — the load-bearing
    // counterpart to the `Unknown` forwards-compat variant in pubky-app-specs.
    let pubky_object = PubkyAppObject::from_resource(&resource, blob.as_slice())
        .map_err(|e| EventProcessorError::SpecValidation(e.to_string()))?;

    let user_id = event.parsed_uri.user_id().clone();
    match (pubky_object, resource) {
        (PubkyAppObject::User(user), Resource::User) => {
            handlers::user::sync_put(user, user_id).await?
        }
        (PubkyAppObject::Post(post), Resource::Post(post_id)) => {
            handlers::post::sync_put(post, user_id, post_id, &ingestor).await?
        }
        (PubkyAppObject::Follow(_follow), Resource::Follow(followee_id)) => {
            handlers::follow::sync_put(user_id, followee_id, &ingestor).await?
        }
        (PubkyAppObject::Mute(_), Resource::Mute(_)) => {
            debug!("Mute events are no longer handled by nexus");
        }
        (PubkyAppObject::Bookmark(bookmark), Resource::Bookmark(bookmark_id)) => {
            handlers::bookmark::sync_put(user_id, bookmark, bookmark_id).await?
        }
        (PubkyAppObject::Tag(tag), Resource::Tag(tag_id)) => {
            if moderation.should_delete(&tag, user_id.clone()) {
                moderation
                    .apply_moderation(tag, event.files_path.clone())
                    .await?
            } else {
                // Route universal tag events (non-pubky.app apps) to sync_put_resource
                // which handles Resource nodes for InternalUnknown/InternalUnknown URIs.
                if let HomeserverParsedUri::UniversalTag { app, .. } = &event.parsed_uri {
                    handlers::tag::sync_put_resource(
                        tag,
                        user_id,
                        tag_id.to_string(),
                        app.clone(),
                        &ingestor,
                    )
                    .await?
                } else {
                    handlers::tag::sync_put(tag, user_id, tag_id.to_string(), &ingestor).await?
                }
            }
        }
        (PubkyAppObject::File(file), Resource::File(file_id)) => {
            handlers::file::sync_put(
                file,
                event.uri.clone(),
                user_id,
                file_id,
                event.files_path.clone(),
                max_file_size,
                &ingestor,
            )
            .await?
        }
        other => debug!("Event type not handled, Resource: {other:?}"),
    }
    Ok(())
}

/// Handles a DEL event by dispatching to the appropriate handler.
pub async fn handle_del_event(
    event: &Event,
    ingestor: Arc<UserIngestor>,
) -> Result<(), EventProcessorError> {
    debug!("Handling DEL event for URI: {}", event.uri);

    let user_id = event.parsed_uri.user_id().clone();
    match event.parsed_uri.resource() {
        Resource::User => handlers::user::del(user_id).await?,
        Resource::Post(post_id) => handlers::post::del(user_id, post_id.clone(), &ingestor).await?,
        Resource::Follow(followee_id) => {
            handlers::follow::del(user_id, followee_id.clone()).await?
        }
        Resource::Mute(_) => debug!("Mute events are no longer handled by nexus"),
        Resource::Bookmark(bookmark_id) => {
            handlers::bookmark::del(user_id, bookmark_id.clone()).await?
        }
        Resource::Tag(_) => handlers::tag::del(&event.uri).await?,
        Resource::File(file_id) => {
            handlers::file::del(&user_id, file_id.clone(), event.files_path.clone()).await?
        }
        other => debug!("DEL event type not handled for resource: {other:?}"),
    }
    Ok(())
}
