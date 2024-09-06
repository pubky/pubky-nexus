use log::{debug, error};
use pubky::PubkyClient;
use uri::ParsedUri;

use crate::models::user::PubkyId;

pub mod handlers;
pub mod processor;
pub mod uri;

#[derive(Debug)]
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
    Bookmark {
        user_id: PubkyId,
        bookmark_id: String,
    },
    Tag {
        user_id: PubkyId,
        tag_id: String,
    },
    // File,
}

// Look for the end pattern after the start index, or use the end of the string if not found
enum EventType {
    Put,
    Del,
}

pub struct Event {
    uri: String,
    event_type: EventType,
    resource_type: ResourceType,
    pubky_client: PubkyClient,
}

impl Event {
    fn from_str(
        line: &str,
        pubky_client: PubkyClient,
    ) -> Result<Option<Self>, Box<dyn std::error::Error + Sync + Send>> {
        debug!("New event: {}", line);
        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        if parts.len() != 2 {
            return Err(format!("Malformed event line: {}", line).into());
        }

        let event_type = match parts[0] {
            "PUT" => EventType::Put,
            "DEL" => EventType::Del,
            _ => {
                return Err(format!("Unknown event type: {}", parts[0]).into());
            }
        };

        let uri = parts[1].to_string();
        let parsed_uri = ParsedUri::try_from(uri.as_str()).unwrap_or_default();

        let resource_type = if uri.ends_with("profile.json") {
            ResourceType::User {
                user_id: parsed_uri.user_id,
            }
        } else if uri.contains("/posts/") {
            ResourceType::Post {
                author_id: parsed_uri.user_id,
                post_id: parsed_uri.post_id.ok_or("Missing post_id")?,
            }
        } else if uri.contains("/follows/") {
            ResourceType::Follow {
                follower_id: parsed_uri.user_id,
                followee_id: parsed_uri.follow_id.ok_or("Missing followee_id")?,
            }
        } else if uri.contains("/bookmarks/") {
            ResourceType::Bookmark {
                user_id: parsed_uri.user_id,
                bookmark_id: parsed_uri.bookmark_id.ok_or("Missing bookmark_id")?,
            }
        } else if uri.contains("/tags/") {
            ResourceType::Tag {
                user_id: parsed_uri.user_id,
                tag_id: parsed_uri.tag_id.ok_or("Missing tag_id")?,
            }
        } else {
            // Handle other resource types
            return Err(format!("Unrecognized resource in URI: {}", uri).into());
        };

        Ok(Some(Event {
            uri,
            event_type,
            resource_type,
            pubky_client,
        }))
    }

    async fn handle(self) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        match self.event_type {
            EventType::Put => self.handle_put_event().await,
            EventType::Del => self.handle_del_event().await,
        }
    }

    async fn handle_put_event(self) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        debug!("Handling PUT event for {:?}", self.resource_type);

        // User PUT event's into the homeserver write new data. We fetch the data
        // for every Resource Type
        let url = reqwest::Url::parse(&self.uri)?;
        let blob = match self.pubky_client.get(url).await {
            Ok(Some(blob)) => blob,
            Ok(None) => {
                error!("No content found at {}", self.uri);
                return Ok(());
            }
            Err(e) => {
                error!("Failed to fetch content at {}: {}", self.uri, e);
                return Err(e.into());
            }
        };

        match self.resource_type {
            ResourceType::User { user_id } => handlers::user::put(user_id, blob).await?,
            ResourceType::Post { author_id, post_id } => {
                handlers::post::put(author_id, post_id, blob).await?
            }
            ResourceType::Follow {
                follower_id,
                followee_id,
            } => handlers::follow::put(follower_id, followee_id, blob).await?,
            ResourceType::Bookmark {
                user_id,
                bookmark_id,
            } => handlers::bookmark::put(user_id, bookmark_id, blob).await?,
            ResourceType::Tag { user_id, tag_id } => {
                handlers::tag::put(user_id, tag_id, blob).await?
            }
        }

        Ok(())
    }

    async fn handle_del_event(self) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
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
            ResourceType::Bookmark {
                user_id,
                bookmark_id,
            } => handlers::bookmark::del(user_id, bookmark_id).await?,
            ResourceType::Tag { user_id, tag_id } => handlers::tag::del(user_id, tag_id).await?,
        }

        Ok(())
    }
}
