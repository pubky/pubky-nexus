use crate::{
    models::{
        post::PostDetails,
        pubky_app::{traits::Validatable, PubkyAppPost, PubkyAppUser},
        user::{PubkyId, UserCounts, UserDetails},
    },
    reindex::{reindex_post, reindex_user},
};
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
        } else if uri.contains("/posts/") {
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

    fn get_user_id(&self) -> Result<PubkyId, Box<dyn std::error::Error + Send + Sync>> {
        // Define the patterns we are looking for in the URI
        let pattern = "pubky://";
        let pub_segment = "/pub/";

        // Find the starting position of the user_id part in the URI
        let start_idx = self
            .uri
            .path
            .find(pattern)
            .map(|start| start + pattern.len())
            .ok_or("Pattern not found in URI")?;

        // Find the ending position of the user_id part
        let end_idx = self.uri.path[start_idx..]
            .find(pub_segment)
            .ok_or("Pub segment not found in URI")?;

        // Extract the user_id and attempt to convert it into a PubkyId
        let user_id_str = &self.uri.path[start_idx..start_idx + end_idx];
        let user_id = PubkyId::try_from(user_id_str)?;

        Ok(user_id)
    }

    fn get_post_id(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Define the pattern we're looking for in the URI
        let post_segment = "/posts/";

        // Find the starting position of the post_id part in the URI
        let start_idx = self
            .uri
            .path
            .find(post_segment)
            .map(|start| start + post_segment.len())
            .ok_or("Post segment not found in URI")?;

        // Extract the post_id from the path
        let post_id = &self.uri.path[start_idx..];

        // Return the post_id as a string
        Ok(post_id.to_string())
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
            ResourceType::User => {
                // Process profile.json and update the databases
                debug!("Processing User resource at {}", self.uri.path);

                // Serialize and validate
                let user = <PubkyAppUser as Validatable>::try_from(&blob)?;

                // Create UserDetails object
                let user_id = self.get_user_id()?;
                let user_details = UserDetails::from_homeserver(user, &user_id).await?;

                // Add new node into the graph
                user_details.save().await?;

                // Reindex to sorted sets and other indexes
                reindex_user(&user_id).await?;
            }
            ResourceType::Post => {
                // Process Post resource and update the databases
                debug!("Processing Post resource at {}", self.uri.path);

                // Serialize and validate
                let post = <PubkyAppPost as Validatable>::try_from(&blob)?;

                // Create UserDetails object
                let author_id = self.get_user_id()?;
                let post_id = self.get_post_id()?;
                let post_details = PostDetails::from_homeserver(post, &author_id, &post_id).await?;

                // Add new post node into the graph
                post_details.save().await?;

                // Reindex to sorted sets and other indexes
                reindex_post(&author_id, &post_id).await?;
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
                let user_details =
                    UserDetails::get_by_id(&self.get_user_id().unwrap_or_default()).await?;

                //TODO: delete from search sorted set, delete user tags, delete followers/following, etc
                match user_details {
                    None => return Ok(()),
                    Some(user_details) => {
                        // TODO create a `deindex` that undoes a `reindex(user_id)`
                        user_details.delete().await?;
                        UserCounts::delete(&user_details.id).await?;
                        // TODO, should also delete from Sorted:Users:Name, MostFollowed and Pioneers
                    }
                }
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
