use nexus_common::db::PubkyConnector;
use nexus_common::models::event::{Event, EventProcessorError, EventType};
use nexus_common::types::DynError;
use pubky_app_specs::{PubkyAppObject, Resource};
use std::sync::Arc;
use tracing::debug;

pub mod handlers;
mod moderation;
pub mod retry;

pub use moderation::Moderation;

pub async fn handle(event: &Event, moderation: Arc<Moderation>) -> Result<(), DynError> {
    match event.event_type {
        EventType::Put => handle_put_event(event, moderation).await,
        EventType::Del => handle_del_event(event).await,
    }?;

    event.store_event().await
}

pub async fn handle_put_event(event: &Event, moderation: Arc<Moderation>) -> Result<(), DynError> {
    debug!("Handling PUT event for URI: {}", event.uri);

    let pubky = PubkyConnector::get()?;
    let response = pubky.public_storage().get(&event.uri).await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<unable to read body>".to_string());

        let err_msg = format!(
            "Fetch resource failed {}: HTTP {status} - {body}",
            event.uri
        );
        return Err(EventProcessorError::client_error(err_msg))?;
    }

    let blob = response.bytes().await?;
    let resource = event.parsed_uri.resource.clone();

    // Use the new importer from pubky-app-specs
    let pubky_object = PubkyAppObject::from_resource(&resource, &blob)?;

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
            if moderation.should_delete(&tag, user_id.clone()).await {
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
                event.files_path.clone(),
            )
            .await?
        }
        other => debug!("Event type not handled, Resource: {other:?}"),
    }
    Ok(())
}

/// Handles a PUT event by fetching the blob from the homeserver
/// and using the importer to convert it to a PubkyAppObject.
pub async fn handle_del_event(event: &Event) -> Result<(), DynError> {
    debug!("Handling DEL event for URI: {}", event.uri);

    let user_id = event.parsed_uri.user_id.clone();
    match &event.parsed_uri.resource {
        Resource::User => handlers::user::del(user_id).await?,
        Resource::Post(post_id) => handlers::post::del(user_id, post_id.clone()).await?,
        Resource::Follow(followee_id) => {
            handlers::follow::del(user_id, followee_id.clone()).await?
        }
        Resource::Mute(muted_id) => handlers::mute::del(user_id, muted_id.clone()).await?,
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
