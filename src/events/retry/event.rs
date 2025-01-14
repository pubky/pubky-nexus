use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{events::error::EventProcessorError, types::DynError, RedisOps};

pub const RETRY_MAMAGER_PREFIX: &str = "RetryManager";
pub const RETRY_MANAGER_EVENTS_INDEX: [&str; 1] = ["events"];
pub const RETRY_MANAGER_STATE_INDEX: [&str; 1] = ["state"];
pub const HOMESERVER_PROTOCOL: &str = "pubky:";
pub const HOMESERVER_PUBLIC_REPOSITORY: &str = "pub";
pub const HOMESERVER_APP_REPOSITORY: &str = "pubky.app";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryEvent {
    // Number of retries attempted
    pub retry_count: u32,
    // This determines how the event should be processed during the retry process
    pub error_type: EventProcessorError,
}

#[async_trait]
impl RedisOps for RetryEvent {
    async fn prefix() -> String {
        String::from(RETRY_MAMAGER_PREFIX)
    }
}

impl RetryEvent {
    pub fn new(error_type: EventProcessorError) -> Self {
        Self {
            retry_count: 0,
            error_type,
        }
    }

    /// It processes a homeserver URI and extracts specific components to form a index key
    /// in the format `"{pubkyId}:{repository_model}:{event_id}"`
    /// # Parameters
    /// - `event_uri`: A string slice representing the event URI to be processed
    pub fn generate_index_key(event_uri: &str) -> Option<String> {
        let parts: Vec<&str> = event_uri.split('/').collect();
        // Ensure the URI structure matches the expected format
        if parts.first() != Some(&HOMESERVER_PROTOCOL)
            || parts.get(3) != Some(&HOMESERVER_PUBLIC_REPOSITORY)
            || parts.get(4) != Some(&HOMESERVER_APP_REPOSITORY)
        {
            return None;
        }

        match parts.as_slice() {
            // Regular PubkyApp URIs
            [_, _, pubky_id, _, _, domain, event_id] => {
                Some(format!("{}:{}:{}", pubky_id, domain, event_id))
            }
            // PubkyApp user profile URI (profile.json)
            [_, _, pubky_id, _, _, "profile.json"] => {
                Some(format!("{}:user:profile.json", pubky_id))
            }
            _ => None,
        }
    }

    /// Stores an event in both a sorted set and a JSON index in Redis.
    /// It adds an event line to a Redis sorted set with a timestamp-based score
    /// and also stores the event details in a separate JSON index for retrieval.
    /// # Arguments
    /// * `event_line` - A `String` representing the event line to be indexed.
    pub async fn put_to_index(&self, event_line: String) -> Result<(), DynError> {
        Self::put_index_sorted_set(
            &RETRY_MANAGER_EVENTS_INDEX,
            // NOTE: Don't know if we should use now timestamp or the event timestamp
            &[(Utc::now().timestamp_millis() as f64, &event_line)],
            Some(RETRY_MAMAGER_PREFIX),
            None,
        )
        .await?;

        let index = &[RETRY_MANAGER_STATE_INDEX, [&event_line]].concat();
        self.put_index_json(index, None).await?;

        Ok(())
    }

    /// Checks if a specific event exists in the Redis sorted set
    /// # Arguments
    /// * `event_index` - A `&str` representing the event index to check
    pub async fn check_uri(event_index: &str) -> Result<Option<isize>, DynError> {
        if let Some(post_details) = Self::check_sorted_set_member(
            Some(RETRY_MAMAGER_PREFIX),
            &RETRY_MANAGER_EVENTS_INDEX,
            &[event_index],
        )
        .await?
        {
            return Ok(Some(post_details));
        }
        Ok(None)
    }

    /// Retrieves an event from the JSON index in Redis based on its index
    /// # Arguments
    /// * `event_index` - A `&str` representing the event index to retrieve
    pub async fn get_from_index(event_index: &str) -> Result<Option<Self>, DynError> {
        let mut found_event = None;
        let index = &[RETRY_MANAGER_STATE_INDEX, [event_index]].concat();
        if let Some(fail_event) = Self::try_from_index_json(index).await? {
            found_event = Some(fail_event);
        }
        Ok(found_event)
    }
}
