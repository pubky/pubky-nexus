use errors::EventProcessorError;
use nexus_common::db::PubkyClient;
use nexus_common::types::DynError;
use pubky_app_specs::{ParsedUri, PubkyAppObject, Resource};
use serde::{Deserialize, Serialize};
use std::{fmt, path::PathBuf, sync::Arc};
use tracing::debug;

pub mod errors;
pub mod handlers;
mod moderation;
pub mod retry;

pub use moderation::Moderation;

// Look for the end pattern after the start index, or use the end of the string if not found
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EventType {
    Put,
    Del,
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let upper_case_str = match self {
            EventType::Put => "PUT",
            EventType::Del => "DEL",
        };
        write!(f, "{upper_case_str}")
    }
}

#[derive(Debug, Clone)]
pub struct Event {
    pub uri: String,
    pub event_type: EventType,
    pub parsed_uri: ParsedUri,
    pub files_path: PathBuf,
}

impl Event {
    pub fn parse_event(line: &str, files_path: PathBuf) -> Result<Option<Self>, DynError> {
        debug!("New event: {}", line);
        let parts: Vec<&str> = line.split(' ').collect();
        if parts.len() != 2 {
            return Err(EventProcessorError::InvalidEventLine {
                message: format!("Malformed event line, {line}"),
            }
            .into());
        }

        let event_type = match parts[0] {
            "PUT" => EventType::Put,
            "DEL" => EventType::Del,
            other => {
                return Err(EventProcessorError::InvalidEventLine {
                    message: format!("Unknown event type: {other}"),
                }
                .into())
            }
        };

        // Validate and parse the URI using pubky-app-specs
        let uri = parts[1].to_string();
        let parsed_uri = ParsedUri::try_from(uri.as_str()).map_err(|e| {
            {
                EventProcessorError::InvalidEventLine {
                    message: format!("Cannot parse event URI: {e}"),
                }
            }
        })?;

        match parsed_uri.resource {
            // Unknown resource
            Resource::Unknown => {
                return Err(EventProcessorError::InvalidEventLine {
                    message: format!("Unknown resource in URI: {uri}"),
                }
                .into())
            }
            // Known resources not handled by Nexus
            Resource::LastRead | Resource::Feed(_) | Resource::Blob(_) => return Ok(None),
            _ => (),
        };

        Ok(Some(Event {
            uri,
            event_type,
            parsed_uri,
            files_path,
        }))
    }

    pub async fn handle(self, moderation: Arc<Moderation>) -> Result<(), DynError> {
        match self.event_type {
            EventType::Put => self.handle_put_event(moderation).await,
            EventType::Del => self.handle_del_event().await,
        }
    }

    /// Handles a PUT event by fetching the blob from the homeserver
    /// and using the importer to convert it to a PubkyAppObject.
    pub async fn handle_put_event(self, moderation: Arc<Moderation>) -> Result<(), DynError> {
        debug!("Handling PUT event for URI: {}", self.uri);

        let response;
        {
            let pubky_client =
                PubkyClient::get().map_err(|e| EventProcessorError::PubkyClientError {
                    message: e.to_string(),
                })?;

            response = match pubky_client.public_storage().get(&self.uri).await {
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
        let resource = self.parsed_uri.resource;

        // Use the new importer from pubky-app-specs
        let pubky_object = PubkyAppObject::from_resource(&resource, &blob).map_err(|e| {
            EventProcessorError::PubkyClientError {
                message: format!(
                    "The importer could not create PubkyAppObject from Uri and Blob: {e}"
                ),
            }
        })?;

        let user_id = self.parsed_uri.user_id;
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
                    Moderation::apply_moderation(tag, self.files_path).await?
                } else {
                    handlers::tag::sync_put(tag, user_id, tag_id).await?
                }
            }
            (PubkyAppObject::File(file), Resource::File(file_id)) => {
                handlers::file::sync_put(file, self.uri, user_id, file_id, self.files_path).await?
            }
            other => {
                debug!("Event type not handled, Resource: {:?}", other);
            }
        }
        Ok(())
    }

    pub async fn handle_del_event(self) -> Result<(), DynError> {
        debug!("Handling DEL event for URI: {}", self.uri);

        let user_id = self.parsed_uri.user_id;
        match self.parsed_uri.resource {
            Resource::User => handlers::user::del(user_id).await?,
            Resource::Post(post_id) => handlers::post::del(user_id, post_id).await?,
            Resource::Follow(followee_id) => handlers::follow::del(user_id, followee_id).await?,
            Resource::Mute(muted_id) => handlers::mute::del(user_id, muted_id).await?,
            Resource::Bookmark(bookmark_id) => {
                handlers::bookmark::del(user_id, bookmark_id).await?
            }
            Resource::Tag(tag_id) => handlers::tag::del(user_id, tag_id).await?,
            Resource::File(file_id) => {
                handlers::file::del(&user_id, file_id, self.files_path).await?
            }
            other => {
                debug!("DEL event type not handled for resource: {:?}", other);
            }
        }
        Ok(())
    }
}
