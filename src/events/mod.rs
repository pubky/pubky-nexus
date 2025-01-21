use std::fmt;

use crate::{
    db::connectors::pubky::PubkyConnector,
    types::{DynError, PubkyId},
};
use error::EventProcessorError;
use log::debug;
use reqwest;
use serde::{Deserialize, Serialize};
use uri::ParsedUri;

pub mod error;
pub mod handlers;
pub mod processor;
pub mod retry;
pub mod uri;

#[derive(Debug, Clone)]
enum ResourceType {
    User {
        user_id: PubkyId,
    },
    Post {
        author_id: PubkyId,
        post_id: String,
    },
    Follow {
        follower_id: PubkyId,
        followee_id: PubkyId,
    },
    Mute {
        user_id: PubkyId,
        muted_id: PubkyId,
    },
    Bookmark {
        user_id: PubkyId,
        bookmark_id: String,
    },
    Tag {
        user_id: PubkyId,
        tag_id: String,
    },
    File {
        user_id: PubkyId,
        file_id: String,
    },
}

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
        write!(f, "{}", upper_case_str)
    }
}

#[derive(Debug, Clone)]
pub struct Event {
    uri: String,
    event_type: EventType,
    resource_type: ResourceType,
}

impl Event {
    pub fn parse_event(line: &str) -> Result<Option<Self>, DynError> {
        debug!("New event: {}", line);
        let parts: Vec<&str> = line.split(' ').collect();
        if parts.len() != 2 {
            return Err(EventProcessorError::InvalidEvent {
                message: format!("Malformed event line: {}", line),
            }
            .into());
        }

        let event_type = match parts[0] {
            "PUT" => EventType::Put,
            "DEL" => EventType::Del,
            _ => {
                return Err(EventProcessorError::InvalidEvent {
                    message: format!("Unknown event type: {}", parts[0]),
                }
                .into())
            }
        };

        let uri = parts[1].to_string();
        let parsed_uri = ParsedUri::try_from(uri.as_str()).unwrap_or_default();

        //TODO: This conversion to a match statement that only uses IF conditions is silly.
        // We could be patter matching the split test for "posts", "follows", etc maybe?
        let resource_type = match uri {
            _ if uri.ends_with("/pub/pubky.app/profile.json") => ResourceType::User {
                user_id: parsed_uri.user_id,
            },
            _ if uri.contains("/posts/") => ResourceType::Post {
                author_id: parsed_uri.user_id,
                post_id: parsed_uri.post_id.ok_or("Missing post_id")?,
            },
            _ if uri.contains("/follows/") => ResourceType::Follow {
                follower_id: parsed_uri.user_id,
                followee_id: parsed_uri.follow_id.ok_or("Missing followee_id")?,
            },
            _ if uri.contains("/mutes/") => ResourceType::Mute {
                user_id: parsed_uri.user_id,
                muted_id: parsed_uri.muted_id.ok_or("Missing muted_id")?,
            },
            _ if uri.contains("/bookmarks/") => ResourceType::Bookmark {
                user_id: parsed_uri.user_id,
                bookmark_id: parsed_uri.bookmark_id.ok_or("Missing bookmark_id")?,
            },
            _ if uri.contains("/tags/") => ResourceType::Tag {
                user_id: parsed_uri.user_id,
                tag_id: parsed_uri.tag_id.ok_or("Missing tag_id")?,
            },
            _ if uri.contains("/files/") => ResourceType::File {
                user_id: parsed_uri.user_id,
                file_id: parsed_uri.file_id.ok_or("Missing file_id")?,
            },
            _ if uri.contains("/blobs") => return Ok(None),
            _ if uri.contains("/last_read") => return Ok(None),
            _ if uri.contains("/settings") => return Ok(None),
            _ => {
                return Err(EventProcessorError::InvalidEvent {
                    message: format!("Unrecognized resource in URI: {}", uri),
                }
                .into())
            }
        };

        Ok(Some(Event {
            uri,
            event_type,
            resource_type,
        }))
    }

    pub async fn handle(self) -> Result<(), DynError> {
        match self.event_type {
            EventType::Put => self.handle_put_event().await,
            EventType::Del => self.handle_del_event().await,
        }
    }

    async fn handle_put_event(self) -> Result<(), DynError> {
        debug!("Handling PUT event for {:?}", self.resource_type);

        // User PUT event's into the homeserver write new data. We fetch the data
        // for every Resource Type
        let url = reqwest::Url::parse(&self.uri)?;
        let pubky_client = PubkyConnector::get_pubky_client()?;

        let response = match pubky_client.get(url).send().await {
            Ok(response) => response,
            Err(e) => {
                return Err(EventProcessorError::PubkyClientError {
                    message: format!("{}", e),
                }
                .into())
            }
        };

        let blob = response.bytes().await?;

        match self.resource_type {
            ResourceType::User { user_id } => handlers::user::put(user_id, &blob).await?,
            ResourceType::Post { author_id, post_id } => {
                handlers::post::put(author_id, post_id, &blob).await?
            }
            ResourceType::Follow {
                follower_id,
                followee_id,
            } => handlers::follow::put(follower_id, followee_id, &blob).await?,
            ResourceType::Mute { user_id, muted_id } => {
                handlers::mute::put(user_id, muted_id, &blob).await?
            }
            ResourceType::Bookmark {
                user_id,
                bookmark_id,
            } => handlers::bookmark::put(user_id, bookmark_id, &blob).await?,
            ResourceType::Tag { user_id, tag_id } => {
                handlers::tag::put(user_id, tag_id, &blob).await?
            }
            ResourceType::File { user_id, file_id } => {
                handlers::file::put(self.uri, user_id, file_id, &blob).await?
            }
        }

        Ok(())
    }

    async fn handle_del_event(self) -> Result<(), DynError> {
        debug!("Handling DEL event for {:?}", self.resource_type);

        match self.resource_type {
            ResourceType::User { user_id } => handlers::user::del(user_id).await?,
            ResourceType::Post { author_id, post_id } => {
                handlers::post::del(author_id, post_id).await?
            }
            ResourceType::Follow {
                follower_id,
                followee_id,
            } => handlers::follow::del(follower_id, followee_id).await?,
            ResourceType::Mute { user_id, muted_id } => {
                handlers::mute::del(user_id, muted_id).await?
            }
            ResourceType::Bookmark {
                user_id,
                bookmark_id,
            } => handlers::bookmark::del(user_id, bookmark_id).await?,
            ResourceType::Tag { user_id, tag_id } => handlers::tag::del(user_id, tag_id).await?,
            ResourceType::File { user_id, file_id } => {
                handlers::file::del(&user_id, file_id).await?
            }
        }

        Ok(())
    }
}
