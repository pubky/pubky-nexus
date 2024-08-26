use crate::models::user::UserDetails;
use log::{debug, error, info};
use pubky::PubkyClient;

pub mod processor;

enum ResourceType {
    User,
    Post,
    // Follow,
    // File,
    // Bookmark,
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
        } else if uri.contains("/post/") {
            ResourceType::Post
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

    fn user_id(&self) -> Option<String> {
        // Extract the part of the URI between "pubky://" and "/pub/". That's the user_id.
        let pattern = "pubky://";
        let pub_segment = "/pub/";

        if let Some(start) = self.uri.path.find(pattern) {
            let start_idx = start + pattern.len();
            if let Some(end_idx) = self.uri.path[start_idx..].find(pub_segment) {
                return Some(self.uri.path[start_idx..start_idx + end_idx].to_string());
            }
        }

        None
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
        let content = match self.pubky_client.get(url).await {
            Ok(Some(content)) => content,
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
            ResourceType::User => {
                // Process profile.json and update the databases
                debug!("Processing User resource at {}", self.uri.path);

                // Implement constructor that writes into the DBs
                let user_details = match self.user_id() {
                    None => return Ok(()),
                    Some(user_id) => UserDetails::from_homeserver(&user_id, &content).await?,
                };

                if let Some(user_details) = user_details {
                    user_details.save().await?;
                }
            }
            ResourceType::Post => {
                // Process Post resource and update the databases
                debug!("Processing Post resource at {}", self.uri.path);
                // Implement constructor that writes into the DBs
                // post_details = PostDetails::from_homeserver(&content).await?;
                // post_details.save()
            }
        }

        Ok(())
    }

    async fn handle_del_event(&self) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        debug!("Handling DEL event for {}", self.uri.path);
        match self.uri.resource_type {
            ResourceType::User => {
                // Handle deletion of profile.json from databases
                debug!("Deleting User resource at {}", self.uri.path);
                // Implement your deletion logic here
            }
            ResourceType::Post => {
                // Handle deletion of Post resource from databases
                debug!("Deleting Post resource at {}", self.uri.path);
                // Implement your deletion logic here
            }
        }

        Ok(())
    }
}
