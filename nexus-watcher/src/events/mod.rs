use nexus_common::{db::PubkyConnector, models::user::UserIngestor};
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
use crate::service::indexer::METER_NAME;
use nexus_common::models::takedown::is_taken_down;
use nexus_common::WatcherConfig;
use opentelemetry::{global, metrics::Counter};
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

mod fetch;
pub mod handlers;
mod moderation;
pub mod retry;

pub(crate) use fetch::{
    fetch_capped, format_error_body, read_stream_capped, MAX_ERROR_BODY, MAX_EVENTS_BODY,
    MAX_RESOURCE_SIZE,
};
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
    max_file_size: u64,

    /// Local files directory on Nexus used for file-backed events.
    files_path: PathBuf,
}

impl DefaultEventHandler {
    pub fn new(
        moderation: Arc<Moderation>,
        ingestor: Arc<UserIngestor>,
        max_file_size: u64,
        files_path: PathBuf,
    ) -> Self {
        Self {
            moderation,
            ingestor,
            max_file_size,
            files_path,
        }
    }

    /// Builds a handler, deriving its moderation rules and user ingestor from config.
    pub fn from_config(config: &WatcherConfig) -> Self {
        Self::new(
            Moderation::from_config(config),
            Arc::new(UserIngestor::from_config(&config.stack)),
            config.max_file_size,
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
                    self.max_file_size,
                    self.files_path.as_path(),
                    self.moderation.clone(),
                    self.ingestor.clone(),
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

/// Fetches the resource an event addresses, capped at [`MAX_RESOURCE_SIZE`].
async fn fetch_resource(uri: &str) -> Result<Vec<u8>, EventProcessorError> {
    let pubky = PubkyConnector::get()?;
    let response = pubky.public_storage().get(uri).await?;

    if !response.status().is_success() {
        let status = response.status();
        let (body, _exceeded) = read_stream_capped(response.bytes_stream(), MAX_ERROR_BODY)
            .await
            .unwrap_or_default();
        let body = format_error_body(&body, MAX_ERROR_BODY);

        let err_msg = format!("Fetch resource failed {uri}: HTTP {status} - {body}");
        return Err(EventProcessorError::client_error(err_msg));
    }

    fetch_capped(response, MAX_RESOURCE_SIZE as u64).await
}

/// Handles a PUT event: fetch, translate, dispatch to the matching handler.
pub async fn handle_put_event(
    event: &Event,
    max_file_size: u64,
    files_path: &Path,
    moderation: Arc<Moderation>,
    ingestor: Arc<UserIngestor>,
) -> Result<(), EventProcessorError> {
    // Whatever order events arrive in, a record the moderator took down stays down. A
    // copy left behind by an earlier attempt whose removal failed is removed here too.
    if remove_if_taken_down(&event.uri, files_path, &ingestor).await? {
        TAKEN_DOWN.add(1, &[]);
        debug!(event.uri = %event.uri, "Record was taken down by the moderator; skipping event");
        return Ok(());
    }

    let blob = fetch_resource(&event.uri).await?;

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
        TranslatedPut::Tag { key, tag, app } => {
            if moderation.should_delete(&tag.label, &key.owner_id) {
                moderation
                    .apply_moderation(&tag.label, tag.tagged, &event.uri, files_path, &ingestor)
                    .await
            } else if let Some(app) = app {
                // Another app's universal tag: `sync_put_resource` also makes Resource nodes
                // for targets that are neither a post nor a user.
                handlers::tag::sync_put_resource(tag, key, app, &ingestor).await
            } else {
                handlers::tag::sync_put(tag, key, &ingestor).await
            }
        }
        TranslatedPut::File { key, file } => {
            handlers::file::sync_put(file, key, files_path, max_file_size, &ingestor).await
        }
    };
    outcome?;

    // Checked again once written: see `remove_if_taken_down`.
    remove_if_taken_down(&event.uri, files_path, &ingestor).await?;
    Ok(())
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
        TranslatedDel::Tag { key, app } => handlers::tag::del(key, app).await,
        TranslatedDel::Bookmark(key) => handlers::bookmark::del(key).await,
        TranslatedDel::File(key) => handlers::file::del(key, files_path).await,
    }
}
