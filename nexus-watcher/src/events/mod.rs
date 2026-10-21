use nexus_common::{db::PubkyConnector, models::user::UserIngestor};
pub mod event;

pub use event::{Event, EventType, ParseResult};

use crate::errors::EventProcessorError;
use crate::service::indexer::METER_NAME;
use nexus_common::models::takedown::is_taken_down;
use nexus_common::WatcherConfig;
use opentelemetry::{global, metrics::Counter};
use pubky_app_specs::{ExtendedParsedUri, PubkyAppObject, Resource};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
};
use tracing::debug;

/// Counter for events skipped because their author is outside the scope.
///
/// The runners only read in-scope users, so this stays near zero; a steady
/// count means events of out-of-scope users are still reaching dispatch.
static OUT_OF_SCOPE: LazyLock<Counter<u64>> = LazyLock::new(|| {
    global::meter(METER_NAME)
        .u64_counter("watcher.events.out_of_scope")
        .with_description("Events skipped because their author is outside the scope")
        .build()
});

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
            Arc::new(UserIngestor::from_watcher_config(config)),
            config.max_file_size,
            config.stack.files_path.clone(),
        )
    }
}

#[async_trait::async_trait]
impl EventHandler for DefaultEventHandler {
    async fn handle(&self, event: &Event) -> Result<(), EventProcessorError> {
        // Safety net for events of authors outside the scope (queued for retry, or read
        // from a homeserver's whole feed): skipped, neither indexed nor retried. A stub or
        // former member that moved onto a listed homeserver is let in first.
        let author_id = event.parsed_uri.user_id();
        if !self.ingestor.scope().is_in_scope(author_id, None).await?
            && !self.ingestor.admit_if_moved_in(author_id).await?
        {
            OUT_OF_SCOPE.add(1, &[]);
            debug!(event.uri = %event.uri, "Author is outside the scope; skipping event");
            return Ok(());
        }

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
        return Err(EventProcessorError::client_error(err_msg));
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
                    .apply_moderation(tag, &event.uri, files_path, &ingestor)
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
                max_file_size,
                &ingestor,
            )
            .await?
        }
        other => debug!(?other, "Event type not handled"),
    }

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
    // Boxed: the delete handlers' futures would otherwise nest inside every PUT's.
    match Box::pin(Moderation::remove_taken_down(uri, files_path, ingestor)).await {
        Ok(()) | Err(EventProcessorError::SkipIndexing) => Ok(true),
        Err(e) => Err(e),
    }
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
