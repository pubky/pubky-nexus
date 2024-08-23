use crate::{models::user::UserDetails, Config};
use log::{debug, error, info};
use pkarr::mainline::Testnet;
use pubky::PubkyClient;
use reqwest::Client;
use tokio::task::JoinHandle;

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

struct Event {
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

pub struct EventProcessor {
    pubky_client: PubkyClient,
    http_client: Client,
    homeserver_url: String, // Ideally should only need the homeserver_pk
    cursor: String,
    limit: u32,
}

impl EventProcessor {
    pub async fn new(config: &Config) -> Self {
        let pubky_client = match config.testnet {
            true => {
                let testnet = Testnet {
                    bootstrap: vec![config.bootstrap.clone()],
                    nodes: vec![],
                };
                PubkyClient::test(&testnet)
            }
            false => PubkyClient::new(),
        };

        Self {
            pubky_client,
            http_client: Client::new(),
            homeserver_url: config.homeserver_url.clone(),
            cursor: "0".to_string(),
            limit: config.events_limit,
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let lines = { self.poll_events().await.unwrap_or_default() };
        if let Some(lines) = lines {
            self.process_event_lines(lines).await?;
        };
        Ok(())
    }

    async fn poll_events(&mut self) -> Result<Option<Vec<String>>, Box<dyn std::error::Error>> {
        let res = self
            .http_client
            .get(format!(
                "{}/events/?cursor={}&limit={}",
                self.homeserver_url, self.cursor, self.limit
            ))
            .send()
            .await?
            .text()
            .await?;

        let lines: Vec<String> = res.trim().split('\n').map(|s| s.to_string()).collect();

        if lines.len() == 1 && lines[0].is_empty() {
            info!("No new events");
            Ok(None)
        } else {
            Ok(Some(lines))
        }
    }

    async fn process_event_lines(
        &mut self,
        lines: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut handles: Vec<JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>> =
            Vec::new();

        for line in &lines {
            if line.starts_with("cursor:") {
                if let Some(cursor) = line.strip_prefix("cursor: ") {
                    self.cursor = cursor.to_string();
                    info!("Cursor for the next request: {}", cursor);
                }
            } else if let Some(event) = Event::from_str(line, self.pubky_client.clone()) {
                // Spawn a new task for each event
                let handle = tokio::spawn(async move { event.handle().await });
                handles.push(handle);
            }
        }

        // Await all tasks concurrently
        for handle in handles {
            handle.await??;
        }

        Ok(())
    }
}
