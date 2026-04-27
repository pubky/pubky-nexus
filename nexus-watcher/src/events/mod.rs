use nexus_common::db::PubkyConnector;
use nexus_common::models::event::{Event, EventProcessorError, EventType};
use nexus_common::universal_tag::homeserver_parsed_uri::HomeserverParsedUri;
use pubky_app_specs::{PubkyAppObject, Resource};
use std::sync::Arc;
use tracing::debug;

pub mod handlers;
mod moderation;
pub mod retry;

pub use moderation::Moderation;

/// Trait for handling events.
///
/// This trait abstracts event handling logic to allow for flexible implementations,
/// including mocked versions for testing.
#[async_trait::async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle(&self, event: &Event) -> Result<(), EventProcessorError>;
}

/// Default implementation of `EventHandler` that uses the actual event handling logic.
pub struct DefaultEventHandler {
    moderation: Arc<Moderation>,
}

impl DefaultEventHandler {
    pub fn new(moderation: Arc<Moderation>) -> Self {
        Self { moderation }
    }
}

#[async_trait::async_trait]
impl EventHandler for DefaultEventHandler {
    async fn handle(&self, event: &Event) -> Result<(), EventProcessorError> {
        match event.event_type {
            EventType::Put => handle_put_event(event, self.moderation.clone()).await,
            EventType::Del => handle_del_event(event).await,
        }?;

        event.store_event().await?;
        Ok(())
    }
}

pub async fn handle_put_event(
    event: &Event,
    moderation: Arc<Moderation>,
) -> Result<(), EventProcessorError> {
    debug!("Handling PUT event for URI: {}", event.uri);

    let pubky = PubkyConnector::get()?;
    let response = pubky.public_storage().get(&event.uri).await?;

    let blob = response
        .bytes()
        .await
        .map_err(|e| EventProcessorError::client_error(e.to_string()))?;
    let resource = event.parsed_uri.resource().clone();

    // Use the new importer from pubky-app-specs
    let pubky_object =
        PubkyAppObject::from_resource(&resource, &blob).map_err(EventProcessorError::generic)?;

    let user_id = event.parsed_uri.user_id().clone();
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
        (PubkyAppObject::Mute(_), Resource::Mute(_)) => {
            debug!("Mute events are no longer handled by nexus");
        }
        (PubkyAppObject::Bookmark(bookmark), Resource::Bookmark(bookmark_id)) => {
            handlers::bookmark::sync_put(user_id, bookmark, bookmark_id).await?
        }
        (PubkyAppObject::Tag(tag), Resource::Tag(tag_id)) => {
            if moderation.should_delete(&tag, user_id.clone()).await {
                moderation
                    .apply_moderation(tag, event.files_path.clone())
                    .await?
            } else {
                // Route universal tag events (non-pubky.app apps) to sync_put_resource
                // which handles Resource nodes for InternalUnknown/InternalUnknown URIs.
                if let HomeserverParsedUri::UniversalTag { app, .. } = &event.parsed_uri {
                    handlers::tag::sync_put_resource(tag, user_id, tag_id.to_string(), app.clone())
                        .await?
                } else {
                    handlers::tag::sync_put(tag, user_id, tag_id.to_string()).await?
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
            )
            .await?
        }
        other => debug!("Event type not handled, Resource: {other:?}"),
    }
    Ok(())
}

/// Handles a DEL event by dispatching to the appropriate handler.
pub async fn handle_del_event(event: &Event) -> Result<(), EventProcessorError> {
    debug!("Handling DEL event for URI: {}", event.uri);

    let user_id = event.parsed_uri.user_id().clone();
    match event.parsed_uri.resource() {
        Resource::User => handlers::user::del(user_id).await?,
        Resource::Post(post_id) => handlers::post::del(user_id, post_id.clone()).await?,
        Resource::Follow(followee_id) => {
            handlers::follow::del(user_id, followee_id.clone()).await?
        }
        Resource::Mute(_) => debug!("Mute events are no longer handled by nexus"),
        Resource::Bookmark(bookmark_id) => {
            handlers::bookmark::del(user_id, bookmark_id.clone()).await?
        }
        Resource::Tag(tag_id) => handlers::tag::del(user_id, tag_id.clone()).await?,
        Resource::File(file_id) => {
            handlers::file::del(&user_id, file_id.clone(), event.files_path.clone()).await?
        }
        other => debug!("DEL event type not handled for resource: {other:?}"),
    }
    Ok(())
}
