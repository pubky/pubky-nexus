use crate::models::{
    file::FileDetails,
    homeserver::{HomeserverFile, HomeserverUser},
    traits::Collection,
    user::{UserCounts, UserDetails},
};
use log::{debug, error, info};
use pubky::PubkyClient;

pub mod processor;

pub enum ResourceType {
    User,
    Post,
    // Follow,
    File,
    // Bookmark,
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
    pub user_id: String,
    pub event_type: EventType,
    pub uri: Uri,
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

        let resource_type = match uri {
            _ if uri.ends_with("profile.json") => ResourceType::User,
            _ if uri.contains("/post/") => ResourceType::Post,
            _ if uri.contains("/file/") => ResourceType::File,
            _ => {
                // Handle other resource types
                error!("Unrecognized resource in URI: {}", uri);
                return None;
            }
        };

        let user_id = match Event::get_user_id(uri) {
            Some(id) => id,
            None => {
                error!("Error getting user_id from event uri. Skipping event.");
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

    fn get_user_id(path: &str) -> Option<String> {
        // Extract the part of the URI between "pubky://" and "/pub/". That's the user_id.
        let pattern = "pubky://";
        let pub_segment = "/pub/";

        if let Some(start) = path.find(pattern) {
            let start_idx = start + pattern.len();
            if let Some(end_idx) = path[start_idx..].find(pub_segment) {
                let user_id = path[start_idx..start_idx + end_idx].to_string();
                return Some(user_id);
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
                let user = HomeserverUser::try_from(&blob).await?;

                // Create UserDetails object
                let user_details = UserDetails::from_homeserver(&self.user_id, user).await?;

                // Index new user event into the Graph and Index
                user_details.save().await?;

                // Add to other sorted sets and indexes
                UserDetails::add_to_sorted_sets(&[Some(user_details)]).await;
            }
            ResourceType::Post => {
                // Process Post resource and update the databases
                debug!("Processing Post resource at {}", self.uri.path);
                // Implement constructor that writes into the DBs
                // post_details = PostDetails::from_homeserver(&blob).await?;
                // post_details.save()
            }
            ResourceType::File => {
                debug!("Processing File resource at {}", self.uri.path);

                // Serialize and validate
                let file_input = HomeserverFile::try_from(&blob).await?;

                // Create FileDetails object
                let file_details =
                    FileDetails::from_homeserver(&self, file_input, &self.pubky_client).await?;

                // Index new user event into the Graph and Index
                file_details.save().await?;
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
                let user_details = UserDetails::get_by_id(&self.user_id).await?;

                //TODO: delete from search sorted set, delete user tags, delete followers/following, etc
                match user_details {
                    None => return Ok(()),
                    Some(user_details) => {
                        user_details.delete().await?;
                        UserCounts::delete(&user_details.id).await?;
                    }
                }
            }
            ResourceType::Post => {
                // Handle deletion of Post resource from databases
                debug!("Deleting Post resource at {}", self.uri.path);
                // Implement your deletion logic here
            }
            ResourceType::File => {
                debug!("Deleting File resource at {}", self.uri.path);
                let file_details =
                    FileDetails::get_file(&FileDetails::file_key_from_uri(&self.uri.path)).await?;

                match file_details {
                    None => return Ok(()),
                    Some(file) => {
                        file.delete().await?;
                    }
                }
            }
        }

        Ok(())
    }
}
