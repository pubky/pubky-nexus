use nexus_common::models::user::UserIngestor;
pub mod event;

pub use event::{Event, EventType, ParseResult};

use crate::errors::EventProcessorError;
use crate::homeserver::HomeserverClient;
use nexus_common::WatcherConfig;
use pubky::PubkyResource;
use pubky_app_specs::{ExtendedParsedUri, PubkyAppObject, Resource};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
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
    ingestor: Arc<UserIngestor>,
    client: Arc<dyn HomeserverClient>,

    /// Local files directory on Nexus used for file-backed events.
    files_path: PathBuf,
}

impl DefaultEventHandler {
    pub fn new(
        moderation: Arc<Moderation>,
        ingestor: Arc<UserIngestor>,
        client: Arc<dyn HomeserverClient>,
        files_path: PathBuf,
    ) -> Self {
        Self {
            moderation,
            ingestor,
            client,
            files_path,
        }
    }

    /// Builds a handler that fetches through `client`, deriving its moderation rules
    /// and user ingestor from config. The ingestor resolves users' homeservers through
    /// `client` too.
    pub fn from_config(config: &WatcherConfig, client: Arc<dyn HomeserverClient>) -> Self {
        let ingestor = UserIngestor::from_config(&config.stack).with_resolver(client.clone());
        Self::new(
            Moderation::from_config(config),
            Arc::new(ingestor),
            client,
            config.stack.files_path.clone(),
        )
    }
}

#[async_trait::async_trait]
impl EventHandler for DefaultEventHandler {
    async fn handle(&self, event: &Event) -> Result<(), EventProcessorError> {
        match event.event_type {
            EventType::Put => {
                handle_put_event(
                    event,
                    self.files_path.as_path(),
                    self.moderation.clone(),
                    self.ingestor.clone(),
                    self.client.as_ref(),
                )
                .await
            }
            EventType::Del => {
                handle_del_event(event, self.files_path.as_path(), self.ingestor.clone()).await
            }
        }?;

        event.to_event_line().store().await?;
        Ok(())
    }
}

/// Handles a PUT event: fetches its resource through `client` and dispatches it to the
/// matching handler.
pub async fn handle_put_event(
    event: &Event,
    files_path: &Path,
    moderation: Arc<Moderation>,
    ingestor: Arc<UserIngestor>,
    client: &dyn HomeserverClient,
) -> Result<(), EventProcessorError> {
    let pubky_resource: PubkyResource = event.uri.parse()?;
    let blob = client.get_resource(&pubky_resource).await?;

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
            handlers::user::sync_put(user, event.uri.clone(), user_id).await?
        }
        (PubkyAppObject::Post(post), Resource::Post(post_id)) => {
            handlers::post::sync_put(post, event.uri.clone(), user_id, post_id, &ingestor).await?
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
            if moderation.should_delete(&tag, &user_id) {
                moderation
                    .apply_moderation(tag, files_path, &ingestor)
                    .await?
            } else {
                // Route universal tag events (non-pubky.app apps) to sync_put_resource
                // which handles Resource nodes for InternalUnknown/InternalUnknown URIs.
                if let ExtendedParsedUri::UniversalTag { app, .. } = &event.parsed_uri {
                    handlers::tag::sync_put_resource(
                        tag,
                        event.uri.clone(),
                        user_id,
                        tag_id.to_string(),
                        app.clone(),
                        &ingestor,
                    )
                    .await?
                } else {
                    handlers::tag::sync_put(
                        tag,
                        event.uri.clone(),
                        user_id,
                        tag_id.to_string(),
                        &ingestor,
                    )
                    .await?
                }
            }
        }
        (PubkyAppObject::File(file), Resource::File(file_id)) => {
            handlers::file::sync_put(
                file,
                event.uri.clone(),
                user_id,
                file_id,
                files_path,
                &ingestor,
                client,
            )
            .await?
        }
        other => debug!(?other, "Event type not handled"),
    }
    Ok(())
}

/// Handles a DEL event by dispatching to the appropriate handler.
pub async fn handle_del_event(
    event: &Event,
    files_path: &Path,
    ingestor: Arc<UserIngestor>,
) -> Result<(), EventProcessorError> {
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
            handlers::file::del(&user_id, file_id.clone(), files_path).await?
        }
        other => debug!(?other, "DEL event type not handled"),
    }
    Ok(())
}
