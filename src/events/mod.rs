use crate::{db::kv::index::sorted_sets::SortOrder, types::PubkyId, RedisOps};
use chrono::Utc;
use log::{debug, error};
use pubky::PubkyClient;
use serde::{Deserialize, Serialize};
use std::error::Error;
use uri::ParsedUri;

pub mod handlers;
pub mod processor;
pub mod uri;

pub const EVENT_ERROR_PREFIX: &str = "error";
pub const EVENT_RECOVERED_PREFIX: &str = "recovered";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum ResourceType {
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
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum EventType {
    Put,
    Del,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Event {
    uri: String,
    event_type: EventType,
    resource_type: ResourceType,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EventInfo {
    event: Event,
    created_at: i64,
    attempts: i32,
    last_attempt_at: Option<i64>,
}

impl RedisOps for EventInfo {}

impl EventInfo {
    pub fn new(event: Event, created_at: i64, attempts: i32, last_attempt_at: Option<i64>) -> Self {
        EventInfo {
            event,
            attempts,
            created_at,
            last_attempt_at,
        }
    }

    pub async fn retry(
        mut self,
        pubky_client: &PubkyClient,
        max_retries: i32,
    ) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        let event_uri = self.event.uri.as_str();
        if let Err(e) = self.clone().event.handle(pubky_client).await {
            self.attempts += 1;
            self.last_attempt_at = Some(Utc::now().timestamp_millis());
            error!(
                "Error while handling retry of event {} with attempt {}: {}",
                event_uri, self.attempts, e
            );

            if self.attempts > max_retries {
                self.put_index_json(&[EVENT_ERROR_PREFIX, event_uri])
                    .await?;
                EventInfo::remove_from_index_multiple_json(&[&[event_uri]]).await?;
                EventFailed::delete(&self).await?;
            } else {
                EventFailed::log(&self).await?;
            }
        } else {
            self.put_index_json(&[EVENT_RECOVERED_PREFIX, event_uri])
                .await?;
            EventInfo::remove_from_index_multiple_json(&[&[event_uri]]).await?;
            EventFailed::delete(&self).await?;
        }
        Ok(())
    }

    pub fn get_attempts(self) -> i32 {
        self.attempts
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EventFailed {}
impl RedisOps for EventFailed {}

impl EventFailed {
    pub async fn log(info: &EventInfo) -> Result<(), Box<dyn Error + Send + Sync>> {
        let score = if info.attempts == 0 {
            info.created_at
        } else {
            info.created_at + ((2 ^ info.attempts) * 1000) as i64
        };
        EventFailed::put_index_sorted_set(
            &[EventFailed::prefix().await.as_str()],
            &[(score as f64, info.event.uri.as_str())],
        )
        .await?;
        Ok(())
    }

    pub async fn delete(info: &EventInfo) -> Result<(), Box<dyn Error + Send + Sync>> {
        EventFailed::remove_from_index_sorted_set(
            &[EventFailed::prefix().await.as_str()],
            &[info.event.uri.as_str()],
        )
        .await?;
        Ok(())
    }

    pub async fn list(
        start: Option<f64>,
        end: Option<f64>,
        skip: Option<usize>,
        limit: Option<usize>,
        sorting: SortOrder,
    ) -> Result<Option<Vec<(String, f64)>>, Box<dyn Error + Send + Sync>> {
        let result = EventFailed::try_from_index_sorted_set(
            &[EventFailed::prefix().await.as_str()],
            start,
            end,
            skip,
            limit,
            sorting,
        )
        .await?;
        Ok(result)
    }
}

impl Event {
    pub fn from_str(line: &str) -> Result<Option<Self>, Box<dyn std::error::Error + Sync + Send>> {
        debug!("New event: {}", line);
        let parts: Vec<&str> = line.split(' ').collect();
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
            _ => {
                error!("Unrecognized resource in URI: {}", uri);
                return Err("Unrecognized resource in URI".into());
            }
        };

        Ok(Some(Event {
            uri,
            event_type,
            resource_type,
        }))
    }

    pub fn new(uri: String, event_type: EventType, resource_type: ResourceType) -> Self {
        Event {
            uri,
            event_type,
            resource_type,
        }
    }

    async fn handle(
        self,
        pubky_client: &PubkyClient,
    ) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        match self.event_type {
            EventType::Put => self.handle_put_event(pubky_client).await,
            EventType::Del => self.handle_del_event().await,
        }
    }

    async fn handle_put_event(
        self,
        pubky_client: &PubkyClient,
    ) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        debug!("Handling PUT event for {:?}", self.resource_type);

        // User PUT event's into the homeserver write new data. We fetch the data
        // for every Resource Type
        let url = reqwest::Url::parse(&self.uri)?;
        let blob = match pubky_client.get(url).await {
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
            ResourceType::Mute { user_id, muted_id } => {
                handlers::mute::put(user_id, muted_id, blob).await?
            }
            ResourceType::Bookmark {
                user_id,
                bookmark_id,
            } => handlers::bookmark::put(user_id, bookmark_id, blob).await?,
            ResourceType::Tag { user_id, tag_id } => {
                handlers::tag::put(user_id, tag_id, blob).await?
            }
            ResourceType::File { user_id, file_id } => {
                handlers::file::put(self.uri, user_id, file_id, blob, pubky_client).await?
            }
        }

        Ok(())
    }

    async fn log_failure(self) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        let now = Utc::now().timestamp_millis();
        let info = EventInfo::new(self.clone(), now, 0, None);
        info.put_index_json(&[self.uri.as_str()]).await?;
        EventFailed::log(&info).await?;
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
