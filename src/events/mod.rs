use handlers::{
    bookmark::parse_bookmark_id, follow::parse_follow_id, post::parse_post_id, user::parse_user_id,
};
use log::{debug, error, info};
use pubky::PubkyClient;

pub mod handlers;
pub mod processor;
enum ResourceType {
    User,
    Post,
    Follow,
    Bookmark,
    // File,
    // Tag,

    // Add more as needed
}

struct Uri {
    resource_type: ResourceType,
    path: String,
}

impl Uri {
    fn new(resource_type: ResourceType, path: &str) -> Self {
        Self {
            resource_type,
            path: path.to_string(),
        }
    }
}

enum EventType {
    Put,
    Del,
}

pub struct Event {
    event_type: EventType,
    uri: Uri,
    pubky_client: PubkyClient,
}

impl Event {
    fn from_str(line: &str, pubky_client: PubkyClient) -> Option<Self> {
        info!("Line {}", line);
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

        let resource_type = if uri.ends_with("profile.json") {
            ResourceType::User
        } else if uri.contains("/posts/") {
            ResourceType::Post
        } else if uri.contains("/follows/") {
            ResourceType::Follow
        } else if uri.contains("/bookmarks/") {
            ResourceType::Bookmark
        } else {
            // Handle other resource types
            error!("Unrecognized resource in URI: {}", uri);
            return None;
        };

        Some(Event {
            event_type,
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
        let url = reqwest::Url::parse(&self.uri.path)?;
        let blob = match self.pubky_client.get(url).await {
            Ok(Some(blob)) => blob,
            Ok(None) => {
                error!("No content found at {}", self.uri.path);
                return Ok(());
            }
            Err(e) => {
                error!("Failed to fetch content at {}: {}", self.uri.path, e);
                return Err(e.into());
            }
        };

        match self.uri.resource_type {
            ResourceType::User => handlers::user::put(parse_user_id(&self.uri.path)?, blob).await?,
            ResourceType::Post => {
                handlers::post::put(
                    parse_user_id(&self.uri.path)?,
                    parse_post_id(&self.uri.path)?,
                    blob,
                )
                .await?
            }
            ResourceType::Follow => {
                handlers::follow::put(
                    parse_user_id(&self.uri.path)?,
                    parse_follow_id(&self.uri.path)?,
                    blob,
                )
                .await?
            }
            ResourceType::Bookmark => {
                handlers::bookmark::put(
                    parse_user_id(&self.uri.path)?,
                    parse_bookmark_id(&self.uri.path)?,
                    blob,
                )
                .await?
            }
        }

        Ok(())
    }

    async fn handle_del_event(&self) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        debug!("Handling DEL event for {}", self.uri.path);
        match self.uri.resource_type {
            ResourceType::User => handlers::user::del(parse_user_id(&self.uri.path)?).await?,
            ResourceType::Post => {
                handlers::post::del(
                    parse_user_id(&self.uri.path)?,
                    parse_post_id(&self.uri.path)?,
                )
                .await?
            }
            ResourceType::Follow => {
                handlers::follow::del(
                    parse_user_id(&self.uri.path)?,
                    parse_follow_id(&self.uri.path)?,
                )
                .await?
            }
            ResourceType::Bookmark => {
                handlers::bookmark::del(
                    parse_user_id(&self.uri.path)?,
                    parse_bookmark_id(&self.uri.path)?,
                )
                .await?
            }
        }

        Ok(())
    }
}
