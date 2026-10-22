use nexus_common::models::user::UserIngestor;
mod classify;
mod epoch_metrics;
pub mod event;
pub mod inputs;
mod route;
pub mod target;
mod translation;

pub use classify::SkipReason;
pub use event::{Event, EventType, ParseResult};
pub use target::EventTarget;

use crate::errors::EventProcessorError;
use crate::homeserver::HomeserverClient;
use crate::METER_NAME;
use nexus_common::models::takedown::is_taken_down;
use nexus_common::WatcherConfig;
use opentelemetry::{global, metrics::Counter};
use pubky::PubkyResource;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
};
use tracing::debug;

/// Counter for events skipped because the moderator took their record down.
static TAKEN_DOWN: LazyLock<Counter<u64>> = LazyLock::new(|| {
    global::meter(METER_NAME)
        .u64_counter("watcher.events.taken_down")
        .with_description("Events skipped because the moderator took their record down")
        .build()
});

pub mod handlers;
mod moderation;
pub mod retry;

use handlers::utils::bounded;
use inputs::{ReferenceTarget, ResourceKey, TagInput};
pub use moderation::Moderation;
use route::resolve_reference;
use translation::{translate_del, translate_put, TranslatedDel, TranslatedPut};

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
        let ingestor = UserIngestor::from_config(&config.stack, client.clone());
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

/// Handles a PUT event: fetches its resource through `client`, translates it and dispatches
/// it to the matching handler.
pub async fn handle_put_event(
    event: &Event,
    files_path: &Path,
    moderation: Arc<Moderation>,
    ingestor: Arc<UserIngestor>,
    client: &dyn HomeserverClient,
) -> Result<(), EventProcessorError> {
    // Whatever order events arrive in, a record the moderator took down stays down. A
    // copy left behind by an earlier attempt whose removal failed is removed here too.
    if remove_if_taken_down(&event.uri, files_path, &ingestor).await? {
        TAKEN_DOWN.add(1, &[]);
        debug!(event.uri = %event.uri, "Record was taken down by the moderator; skipping event");
        return Ok(());
    }

    let pubky_resource: PubkyResource = event.uri.parse()?;
    let blob = client.get_resource(&pubky_resource).await?;

    let outcome = match translate_put(&event.uri, &event.target, &blob)? {
        TranslatedPut::User { user_id, user } => handlers::user::sync_put(user, user_id).await,
        TranslatedPut::Post { key, post } => handlers::post::sync_put(post, key, &ingestor).await,
        TranslatedPut::Follow {
            user_id,
            followee_id,
        } => handlers::follow::sync_put(user_id, followee_id, &ingestor).await,
        TranslatedPut::Bookmark { key, bookmark } => {
            handlers::bookmark::sync_put(bookmark, key).await
        }
        TranslatedPut::Tag { key, tag } => {
            // A moderator tag takes the takedown path, whatever it tags.
            if moderation.should_delete(&tag.label, &key.owner_id) {
                moderation
                    .apply_moderation(&tag.label, tag.tagged, &event.uri, files_path, &ingestor)
                    .await
            } else {
                put_tag(tag, key, &ingestor).await
            }
        }
        TranslatedPut::File { key, file } => {
            handlers::file::sync_put(file, key, files_path, &ingestor, client).await
        }
    };
    outcome?;

    // Checked again once written: see `remove_if_taken_down`.
    remove_if_taken_down(&event.uri, files_path, &ingestor).await?;
    Ok(())
}

/// Indexes a non-moderator tag by what it tags, so the same tag gives the same result from any
/// folder or epoch (`pubky.app/tags`, `social/v1/tags` or another app's folder):
///
/// - a post or a user: a post or user tag ([`handlers::tag::sync_put`]);
/// - a foreign resource or a file, at either epoch's address: a `Resource` tag
///   ([`handlers::tag::sync_put_resource`]);
/// - a tag or anything else: [`EventProcessorError::UnsupportedTarget`], never retried. A
///   `social/vN` target dropped here comes back through a reindex once Nexus speaks that epoch.
pub async fn put_tag(
    tag: TagInput,
    key: ResourceKey,
    ingestor: &UserIngestor,
) -> Result<(), EventProcessorError> {
    match &tag.tagged.target {
        ReferenceTarget::Post(_) | ReferenceTarget::User(_) => {
            handlers::tag::sync_put(tag, key, ingestor).await
        }
        ReferenceTarget::Foreign | ReferenceTarget::File(_) => {
            handlers::tag::sync_put_resource(tag, key).await
        }
        target @ (ReferenceTarget::Tag { .. } | ReferenceTarget::Other) => {
            Err(EventProcessorError::UnsupportedTarget(format!(
                "The tagged resource {} is neither a post, a user, a file nor a foreign resource, instead is: {}",
                bounded(&tag.tagged.uri),
                bounded(&format!("{target:?}"))
            )))
        }
    }
}

/// Removes the record at `uri` if the moderator took it down, and returns whether it did.
///
/// Run before a record is fetched and again once it is written. The moderator's tag may
/// have been read by another loop (primary, external, retry) while the record was being
/// fetched, and found nothing to delete yet: if the takedown is recorded by the second
/// check, the record is removed there; if later, the moderator's own delete runs after
/// the write and removes it. A removal that failed is finished on the event's retry, by
/// the first check. Removing what is already gone does nothing.
pub async fn remove_if_taken_down(
    uri: &str,
    files_path: &Path,
    ingestor: &UserIngestor,
) -> Result<bool, EventProcessorError> {
    if !is_taken_down(uri).await? {
        return Ok(false);
    }
    let target = resolve_reference(uri).target;
    // Boxed: the delete handlers' futures would otherwise nest inside every PUT's.
    match Box::pin(Moderation::remove_taken_down(target, files_path, ingestor)).await {
        Ok(()) | Err(EventProcessorError::SkipIndexing) => Ok(true),
        Err(e) => Err(e),
    }
}

/// Handles a DEL event: translate, dispatch to the matching handler.
pub async fn handle_del_event(
    event: &Event,
    files_path: &Path,
    ingestor: Arc<UserIngestor>,
) -> Result<(), EventProcessorError> {
    match translate_del(&event.target)? {
        TranslatedDel::User { user_id } => handlers::user::del(user_id).await,
        TranslatedDel::Post(key) => handlers::post::del(key, &ingestor).await,
        TranslatedDel::Follow {
            user_id,
            followee_id,
        } => handlers::follow::del(user_id, followee_id).await,
        TranslatedDel::Tag(key) => handlers::tag::del(key).await,
        TranslatedDel::Bookmark(key) => handlers::bookmark::del(key).await,
        TranslatedDel::File(key) => handlers::file::del(key, files_path).await,
    }
}
