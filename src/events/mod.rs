use handlers::{
    bookmark::parse_bookmark_id, file::parse_file_id, follow::parse_follow_id, post::parse_post_id,
};
use log::{debug, error};
use pubky::PubkyClient;

use crate::models::user::PubkyId;

pub mod handlers;
pub mod processor;
pub enum ResourceType {
    User,
    Post,
    Follow,
    Bookmark,
    File,
    // Tag,
    // Add more as needed
}

pub struct Uri {
    pub resource_type: ResourceType,
    pub path: String,
}

impl Uri {
    fn new(resource_type: ResourceType, path: &str) -> Self {
        Self {
            resource_type,
            path: path.to_string(),
        }
    }
}

pub enum EventType {
    Put,
    Del,
}

pub struct Event {
    pub user_id: PubkyId,
    pub event_type: EventType,
    pub uri: Uri,
    pubky_client: PubkyClient,
}

impl Event {
    fn from_str(line: &str, pubky_client: PubkyClient) -> Option<Self> {
        debug!("New event: {}", line);
        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        if parts.len() != 2 {
            error!("Malformed event line: {}", line);
            return None;
        }

        let event_type = match parts[0] {
            "PUT" => EventType::Put,
            "DEL" => EventType::Del,
            _ => {
                error!("Unknown event type: {}", parts[0]);
                return None;
            }
        };

        let uri = parts[1];

        let resource_type = match uri {
            _ if uri.ends_with("profile.json") => ResourceType::User,
            _ if uri.contains("/posts/") => ResourceType::Post,
            _ if uri.contains("/follows/") => ResourceType::Follow,
            _ if uri.contains("/bookmarks/") => ResourceType::Bookmark,
            _ if uri.contains("/files/") => ResourceType::File,
            _ => {
                // Handle other resource types
                error!("Unrecognized resource in URI: {}", uri);
                return None;
            }
        };

        let user_id = match handlers::user::parse_user_id(uri) {
            Ok(id) => id,
            Err(err) => {
                error!(
                    "Error getting user_id from event uri. Skipping event. Details: {:?}",
                    err
                );
                return None;
            }
        };

        Some(Event {
            event_type,
            user_id,
            uri: Uri::new(resource_type, uri),
            pubky_client,
        })
    }

    async fn handle(&self) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        match self.event_type {
            EventType::Put => self.handle_put_event().await,
            EventType::Del => self.handle_del_event().await,
        }
    }

    async fn handle_put_event(&self) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        debug!("Handling PUT event for {}", self.uri.path);

        let uri = &self.uri.path;

        let url = reqwest::Url::parse(uri)?;
        let blob = match self.pubky_client.get(url).await {
            Ok(Some(blob)) => blob,
            Ok(None) => {
                error!("No content found at {}", self.uri.path);
                return Ok(());
            }
            Err(e) => {
                error!("Failed to fetch content at {}: {}", uri, e);
                return Err(e.into());
            }
        };

        match self.uri.resource_type {
            ResourceType::User => handlers::user::put(&self.user_id, blob).await?,
            ResourceType::Post => {
                handlers::post::put(&self.user_id, parse_post_id(uri)?, blob).await?
            }
            ResourceType::Follow => {
                handlers::follow::put(&self.user_id, parse_follow_id(uri)?, blob).await?
            }
            ResourceType::Bookmark => {
                handlers::bookmark::put(&self.user_id, parse_bookmark_id(uri)?, blob).await?
            }
            ResourceType::File => {
                handlers::file::put(
                    self.uri.path.clone(),
                    self.user_id.clone(),
                    parse_file_id(uri)?,
                    blob,
                    &self.pubky_client,
                )
                .await?
            }
        }

        Ok(())
    }

    async fn handle_del_event(&self) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        debug!("Handling DEL event for {}", self.uri.path);

        let uri = &self.uri.path;
        match self.uri.resource_type {
            ResourceType::User => handlers::user::del(&self.user_id).await?,
            ResourceType::Post => handlers::post::del(&self.user_id, parse_post_id(uri)?).await?,
            ResourceType::Follow => {
                handlers::follow::del(&self.user_id, parse_follow_id(uri)?).await?
            }
            ResourceType::Bookmark => {
                handlers::bookmark::del(&self.user_id, parse_bookmark_id(uri)?).await?
            }
            ResourceType::File => handlers::file::del(&self.user_id, parse_file_id(uri)?).await?,
        }

        Ok(())
    }
}
