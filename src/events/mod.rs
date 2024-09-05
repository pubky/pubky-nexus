use handlers::{
    bookmark::parse_bookmark_id, follow::parse_follow_id, post::parse_post_id, tag::parse_tag_id,
    user::parse_user_id,
};
use log::{debug, error};
use pubky::PubkyClient;

pub mod handlers;
pub mod processor;
enum ResourceType {
    User,
    Post,
    Follow,
    Bookmark,
    Tag,
    // File,
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

        let resource_type = if uri.ends_with("profile.json") {
            ResourceType::User
        } else if uri.contains("/posts/") {
            ResourceType::Post
        } else if uri.contains("/follows/") {
            ResourceType::Follow
        } else if uri.contains("/bookmarks/") {
            ResourceType::Bookmark
        } else if uri.contains("/tags/") {
            ResourceType::Tag
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
            ResourceType::User => handlers::user::put(parse_user_id(uri)?, blob).await?,
            ResourceType::Post => {
                handlers::post::put(parse_user_id(uri)?, parse_post_id(uri)?, blob).await?
            }
            ResourceType::Follow => {
                handlers::follow::put(parse_user_id(uri)?, parse_follow_id(uri)?, blob).await?
            }
            ResourceType::Bookmark => {
                handlers::bookmark::put(parse_user_id(uri)?, parse_bookmark_id(uri)?, blob).await?
            }
            ResourceType::Tag => {
                handlers::tag::put(parse_user_id(uri)?, parse_tag_id(uri)?, blob).await?
            }
        }

        Ok(())
    }

    async fn handle_del_event(&self) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        debug!("Handling DEL event for {}", self.uri.path);

        let uri = &self.uri.path;
        match self.uri.resource_type {
            ResourceType::User => handlers::user::del(parse_user_id(uri)?).await?,
            ResourceType::Post => {
                handlers::post::del(parse_user_id(uri)?, parse_post_id(uri)?).await?
            }
            ResourceType::Follow => {
                handlers::follow::del(parse_user_id(uri)?, parse_follow_id(uri)?).await?
            }
            ResourceType::Bookmark => {
                handlers::bookmark::del(parse_user_id(uri)?, parse_bookmark_id(uri)?).await?
            }
            ResourceType::Tag => {
                handlers::tag::del(parse_user_id(uri)?, parse_tag_id(uri)?).await?
            }
        }

        Ok(())
    }
}
