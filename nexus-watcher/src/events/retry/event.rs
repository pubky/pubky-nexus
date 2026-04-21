use async_trait::async_trait;
use nexus_common::db::kv::{RedisResult, SortOrder};
use nexus_common::models::event::EventType;
use nexus_common::models::event::HomeserverParsedUri;
use serde::{Deserialize, Serialize};

use nexus_common::db::RedisOps;

use crate::events::EventProcessorError;

const RETRY_MANAGER_PREFIX: &str = "RetryManager";
const RETRY_MANAGER_EVENTS_INDEX: [&str; 1] = ["events"];
const RETRY_MANAGER_STATE_INDEX: [&str; 1] = ["state"];

/// Represents an event in the retry queue and it is used to manage events that have failed
/// to process and need to be retried
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryEvent {
    /// Retry attempts made for this event
    pub retry_count: u32,
    /// The type of event - needed to reconstruct the event on retry
    pub event_type: EventType,
    /// Original URI - blob is re-fetched on retry
    pub event_uri: String,
    /// Unix ms - when to next attempt (exponential backoff)
    pub next_retry_at: i64,
}

#[async_trait]
impl RedisOps for RetryEvent {
    async fn prefix() -> String {
        String::from(RETRY_MANAGER_PREFIX)
    }
}

impl RetryEvent {
    /// Creates a new RetryEvent
    pub fn new(event_type: EventType, event_uri: String, next_retry_at: i64) -> Self {
        Self {
            retry_count: 0,
            event_type,
            event_uri,
            next_retry_at,
        }
    }

    /// Generates an index key from a parsed URI.
    /// For `AppSpec` variants: `"{user_id}:{resource}"` (e.g., "abc123:posts")
    /// For `UniversalTag` variants: `"{user_id}:{app}/{resource}"` (e.g., "abc123:mapky/tags")
    pub fn generate_index_key(parsed_uri: HomeserverParsedUri) -> String {
        let user_id = parsed_uri.user_id();
        match &parsed_uri {
            HomeserverParsedUri::AppSpec { resource, .. } => {
                format!("{}:{}", user_id, resource)
            }
            HomeserverParsedUri::UniversalTag { app, resource, .. } => {
                format!("{}:{}/{}", user_id, app, resource)
            }
        }
    }

    /// Stores an event in both a sorted set and a JSON index in Redis.
    /// The sorted set uses next_retry_at as the score for efficient retrieval of ready events.
    /// # Arguments
    /// * `resource_key` - A `&str` representing the resource key (used as member in sorted set and JSON key)
    #[tracing::instrument(name = "retry.index.write", skip_all)]
    pub async fn put_to_index(&self, resource_key: &str) -> RedisResult<()> {
        // Add to sorted set with next_retry_at as score
        Self::put_index_sorted_set(
            &RETRY_MANAGER_EVENTS_INDEX,
            &[(self.next_retry_at as f64, resource_key)],
            Some(RETRY_MANAGER_PREFIX),
            None,
        )
        .await?;

        // Store full RetryEvent struct in JSON
        let index: &Vec<&str> = &[RETRY_MANAGER_STATE_INDEX, [resource_key]].concat();
        self.put_index_json(index, None, None).await?;

        Ok(())
    }

    /// Checks if a specific event exists in the Redis sorted set
    /// # Arguments
    /// * `event_index` - A `&str` representing the event index to check
    pub async fn check_uri(event_index: &str) -> Result<Option<isize>, EventProcessorError> {
        Self::check_sorted_set_member(
            Some(RETRY_MANAGER_PREFIX),
            &RETRY_MANAGER_EVENTS_INDEX,
            &[event_index],
        )
        .await
        .map_err(|e| {
            EventProcessorError::internal_error(format!(
                "Could not check uri for event: {event_index}, reason {e}"
            ))
        })
    }

    /// Retrieves an event from the JSON index in Redis based on its index
    /// # Arguments
    /// * `event_index` - A `&str` representing the event index to retrieve
    pub async fn get_from_index(event_index: &str) -> RedisResult<Option<Self>> {
        let index: &Vec<&str> = &[RETRY_MANAGER_STATE_INDEX, [event_index]].concat();
        Self::try_from_index_json(index, None).await
    }

    /// Removes an event from the retry queue (both sorted set and JSON state)
    /// # Arguments
    /// * `resource_key` - A `&str` representing the resource key to remove
    #[tracing::instrument(name = "retry.index.remove", skip_all)]
    pub async fn remove_from_index(resource_key: &str) -> RedisResult<()> {
        // Remove from sorted set
        Self::remove_from_index_sorted_set(
            Some(RETRY_MANAGER_PREFIX),
            &RETRY_MANAGER_EVENTS_INDEX,
            &[resource_key],
        )
        .await?;

        // Remove JSON state
        let index: &Vec<&str> = &[RETRY_MANAGER_STATE_INDEX, [resource_key]].concat();
        Self::remove_from_index_multiple_json(&[index.as_slice()]).await?;

        Ok(())
    }

    /// Fetches events from the retry queue that are ready to be retried (next_retry_at <= now)
    /// Returns Vec<(resource_key, score)> pairs for events ready for retry
    /// # Arguments
    /// * `now` - Current time in milliseconds since epoch
    /// * `limit` - Maximum number of events to fetch per batch
    /// # Returns
    /// A vector of (resource_key, score) pairs, or None if no events found
    #[tracing::instrument(name = "retry.index.fetch_ready", skip_all)]
    pub async fn fetch_ready(
        now: i64,
        limit: Option<usize>,
    ) -> Result<Option<Vec<(String, f64)>>, EventProcessorError> {
        // try_from_index_sorted_set accepts (start, end) but passes them to get_range as (end, start).
        // To fetch events with score <= now (i.e., min=None, max=now), we must pass:
        //   - start param = now (becomes end/max_score in get_range)
        //   - end param   = None (becomes start/min_score in get_range)
        let start_param: Option<f64> = Some(now as f64); // passed as `end` in get_range → acts as max_score
        let end_param: Option<f64> = None; // passed as `start` in get_range → acts as min_score

        Self::try_from_index_sorted_set(
            &RETRY_MANAGER_EVENTS_INDEX,
            start_param,
            end_param,
            Some(0),
            limit,
            SortOrder::Ascending,
            Some(RETRY_MANAGER_PREFIX),
        )
        .await
        .map_err(|e| EventProcessorError::generic(format!("Failed to fetch retry events: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_common::models::event::HomeserverParsedUri;
    use pubky_app_specs::{PubkyId, Resource};

    const TEST_USER_ID: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";

    fn make_appspec_post() -> HomeserverParsedUri {
        let user_id = PubkyId::try_from(TEST_USER_ID).unwrap();
        let resource = Resource::Post("POST123".to_string());
        HomeserverParsedUri::AppSpec { user_id, resource }
    }

    fn make_appspec_tag() -> HomeserverParsedUri {
        let user_id = PubkyId::try_from(TEST_USER_ID).unwrap();
        let resource = Resource::Tag("TAG456".to_string());
        HomeserverParsedUri::AppSpec { user_id, resource }
    }

    fn make_appspec_user() -> HomeserverParsedUri {
        let user_id = PubkyId::try_from(TEST_USER_ID).unwrap();
        let resource = Resource::User;
        HomeserverParsedUri::AppSpec { user_id, resource }
    }

    fn make_universal_tag() -> HomeserverParsedUri {
        let user_id = PubkyId::try_from(TEST_USER_ID).unwrap();
        let resource = Resource::Tag("MYP123".to_string());
        HomeserverParsedUri::UniversalTag {
            user_id,
            app: "mapky".to_string(),
            resource,
            tag_id: "MYP123".to_string(),
        }
    }

    fn make_universal_tag_eventky() -> HomeserverParsedUri {
        let user_id = PubkyId::try_from(TEST_USER_ID).unwrap();
        let resource = Resource::Tag("EVT789".to_string());
        HomeserverParsedUri::UniversalTag {
            user_id,
            app: "eventky.app".to_string(),
            resource,
            tag_id: "EVT789".to_string(),
        }
    }

    #[test]
    fn test_generate_index_key_appspec_post() {
        let parsed = make_appspec_post();
        let key = RetryEvent::generate_index_key(parsed);
        // Resource::Post displays as "posts" (the resource type only)
        assert_eq!(
            key,
            "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo:posts"
        );
    }

    #[test]
    fn test_generate_index_key_appspec_tag() {
        let parsed = make_appspec_tag();
        let key = RetryEvent::generate_index_key(parsed);
        // Resource::Tag displays as "tags" (the resource type only)
        assert_eq!(
            key,
            "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo:tags"
        );
    }

    #[test]
    fn test_generate_index_key_appspec_user() {
        let parsed = make_appspec_user();
        let key = RetryEvent::generate_index_key(parsed);
        // Resource::User displays as "profile.json"
        assert_eq!(
            key,
            "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo:profile.json"
        );
    }

    #[test]
    fn test_generate_index_key_universal_tag() {
        let parsed = make_universal_tag();
        let key = RetryEvent::generate_index_key(parsed);
        // UniversalTag includes the app in the key: "{user_id}:{app}/{resource}"
        assert_eq!(
            key,
            "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo:mapky/tags"
        );
    }

    #[test]
    fn test_generate_index_key_universal_tag_eventky() {
        let parsed = make_universal_tag_eventky();
        let key = RetryEvent::generate_index_key(parsed);
        // UniversalTag includes the app in the key: "{user_id}:{app}/{resource}"
        assert_eq!(
            key,
            "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo:eventky.app/tags"
        );
    }

    #[test]
    fn test_generate_index_key_appspec_vs_universal_tag_different_format() {
        // AppSpec::Tag and UniversalTag::Tag produce different key formats
        let appspec = make_appspec_tag();
        let universal_mapky = make_universal_tag();

        let key_appspec = RetryEvent::generate_index_key(appspec);
        let key_universal = RetryEvent::generate_index_key(universal_mapky);

        // AppSpec uses "{user_id}:{resource}", UniversalTag uses "{user_id}:{app}/{resource}"
        assert_eq!(key_appspec, "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo:tags");
        assert_eq!(key_universal, "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo:mapky/tags");
        assert_ne!(key_appspec, key_universal);
    }

    #[test]
    fn test_generate_index_key_universal_tag_different_apps_different_keys() {
        // UniversalTag variants with different apps should produce different index keys
        // to avoid collisions between events from different apps (e.g., mapky vs eventky)
        let mapky = make_universal_tag();
        let eventky = make_universal_tag_eventky();

        let key_mapky = RetryEvent::generate_index_key(mapky);
        let key_eventky = RetryEvent::generate_index_key(eventky);

        // Different apps should produce different keys
        assert_ne!(key_mapky, key_eventky);
        assert_eq!(key_mapky, "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo:mapky/tags");
        assert_eq!(key_eventky, "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo:eventky.app/tags");
    }
}
