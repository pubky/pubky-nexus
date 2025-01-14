use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{events::error::EventProcessorError, types::DynError, RedisOps};

pub const RETRY_MAMAGER_PREFIX: &str = "RetryManager";
pub const RETRY_MANAGER_EVENTS_INDEX: [&str; 1] = ["events"];
pub const RETRY_MANAGER_STATE_INDEX: [&str; 1] = ["state"];
pub const HOMESERVER_PUBLIC_REPOSITORY: &str = "pub";
pub const HOMESERVER_APP_REPOSITORY: &str = "pubky.app";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryEvent {
    // Number of retries attempted
    pub retry_count: u32,
    // This determines how the event should be processed during the retry process
    pub error_type: EventProcessorError,
}

impl RedisOps for RetryEvent {}

impl RetryEvent {
    pub fn new(error_type: EventProcessorError) -> Self {
        Self {
            retry_count: 0,
            error_type,
        }
    }

    /// It processes a homeserver URI and extracts specific components to form a index key
    /// in the format `"{pubkyId}:{repository_model}/{event_id}"`
    /// # Parameters
    /// - `event_uri`: A string slice representing the event URI to be processed
    pub fn generate_index_key(event_uri: &str) -> Option<String> {
        let parts: Vec<&str> = event_uri.split('/').collect();
        if parts.len() >= 7
            && parts[0] == "pubky:"
            && parts[3] == HOMESERVER_PUBLIC_REPOSITORY
            && parts[4] == HOMESERVER_APP_REPOSITORY
        {
            Some(format!("{}:{}/{}", parts[2], parts[5], parts[6]))
        } else {
            None
        }
    }

    pub async fn put_to_index(&self, event_line: String) -> Result<(), DynError> {
        Self::put_index_sorted_set(
            &RETRY_MANAGER_EVENTS_INDEX,
            // NOTE: Don't know if we should use now timestamp or the event timestamp
            &[(Utc::now().timestamp_millis() as f64, &event_line)],
            Some(RETRY_MAMAGER_PREFIX),
            None,
        )
        .await?;

        let event_serialized = serde_json::to_string(self)?;

        Self::put_index_hash_map(
            Some(RETRY_MAMAGER_PREFIX),
            &RETRY_MANAGER_STATE_INDEX,
            &event_line,
            event_serialized,
        )
        .await?;
        Ok(())
    }

    pub async fn check_uri(event_line: &str) -> Result<Option<isize>, DynError> {
        if let Some(post_details) = Self::check_sorted_set_member(
            Some(RETRY_MAMAGER_PREFIX),
            &RETRY_MANAGER_EVENTS_INDEX,
            &[event_line],
        )
        .await?
        {
            return Ok(Some(post_details));
        }
        Ok(None)
    }

    pub async fn get_from_index(pubky_uri: &str) -> Result<Option<Self>, DynError> {
        let mut found_event = None;
        if let Some(event_state) = Self::get_index_hash_map(
            Some(RETRY_MAMAGER_PREFIX),
            &RETRY_MANAGER_STATE_INDEX,
            pubky_uri,
        )
        .await?
        {
            let event = serde_json::from_str::<Self>(&event_state)?;
            found_event = Some(event);
        }
        Ok(found_event)
    }
}
