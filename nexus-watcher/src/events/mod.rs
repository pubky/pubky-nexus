use nexus_common::{db::PubkyConnector, models::user::UserIngestor};
mod classify;
mod epoch_metrics;
pub mod event;
mod route;
pub mod target;
mod translation;

pub use classify::SkipReason;
pub use event::{Event, EventType, ParseResult};
pub use target::EventTarget;

use crate::errors::EventProcessorError;
use nexus_common::WatcherConfig;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

mod fetch;
pub mod handlers;
mod moderation;
pub mod retry;

pub(crate) use fetch::{
    fetch_capped, format_error_body, read_stream_capped, MAX_ERROR_BODY, MAX_EVENTS_BODY,
    MAX_RESOURCE_SIZE,
};
pub use moderation::Moderation;
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
    let blob = fetch_resource(&event.uri).await?;

    match translate_put(&event.target, &blob)? {
        TranslatedPut::User { user_id, user } => {
            handlers::user::sync_put(user, event.uri.clone(), user_id).await
        }
        TranslatedPut::Post {
            author_id,
            post_id,
            post,
        } => handlers::post::sync_put(post, event.uri.clone(), author_id, post_id, &ingestor).await,
        TranslatedPut::Follow {
            user_id,
            followee_id,
        } => handlers::follow::sync_put(user_id, followee_id, &ingestor).await,
        TranslatedPut::Bookmark {
            user_id,
            bookmark_id,
            bookmark,
        } => handlers::bookmark::sync_put(user_id, bookmark, bookmark_id).await,
        TranslatedPut::Tag {
            tagger_id,
            tag_id,
            tag,
            app,
        } => {
            if moderation.should_delete(&tag.label, &tagger_id) {
                moderation
                    .apply_moderation(&tag.label, &tag.uri, files_path, &ingestor)
                    .await
            } else if let Some(app) = app {
                // Another app's universal tag: `sync_put_resource` also handles Resource
                // nodes for InternalUnknown/External tagged URIs.
                handlers::tag::sync_put_resource(
                    tag,
                    event.uri.clone(),
                    tagger_id,
                    tag_id,
                    app,
                    &ingestor,
                )
                .await
            } else {
                handlers::tag::sync_put(tag, event.uri.clone(), tagger_id, tag_id, &ingestor).await
            }
        }
        TranslatedPut::File {
            user_id,
            file_id,
            file,
        } => {
            handlers::file::sync_put(
                file,
                event.uri.clone(),
                user_id,
                file_id,
                files_path,
                max_file_size,
                &ingestor,
            )
            .await
        }
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
        TranslatedDel::Post { author_id, post_id } => {
            handlers::post::del(author_id, post_id, &ingestor).await
        }
        TranslatedDel::Follow {
            user_id,
            followee_id,
        } => handlers::follow::del(user_id, followee_id).await,
        TranslatedDel::Tag => handlers::tag::del(&event.uri).await,
        TranslatedDel::Bookmark {
            user_id,
            bookmark_id,
        } => handlers::bookmark::del(user_id, bookmark_id).await,
        TranslatedDel::File { user_id, file_id } => {
            handlers::file::del(&user_id, file_id, files_path).await
        }
    }
}
