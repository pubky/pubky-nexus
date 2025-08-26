use crate::db::RedisOps;
use crate::types::DynError;
use pubky_app_specs::PubkyId;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents a event_list with its public key, URL, and cursor.
#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
pub struct EventsList {
    pub events: Vec<String>,
    pub cursor: String,
    pub limit: usize,
}

impl RedisOps for EventsList {}

impl EventsList {
    pub async fn new(id: PubkyId) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        let hs = EventsList {
            events: vec![],
            limit: 0,
            cursor: "0000000000000".to_string(),
        };
        // Store event_list with initial cursor in Index
        hs.put_to_index().await?;
        Ok(hs)
    }

    /// Retrieves the homeserver from Redis.
    pub async fn get_from_index(id: &str) -> Result<Option<Self>, DynError> {
        if let Some(homeserver) = Self::try_from_index_json(&[id], None).await? {
            return Ok(Some(homeserver));
        }
        Ok(None)
    }

    /// Stores the event_list in Redis.
    pub async fn put_to_index(&self) -> Result<(), DynError> {
        self.put_index_json(&[&"foo"], None, None).await?;
        Ok(())
    }

    /// Reads from Redis db
    // TODO: (524)
    // By Chirs:
    // > The implementation consist of a new Redis SortedSet where we write down timestamp and PUT/DEL pubky://{pubky}/object (to be crockford32 encoded as in the example above).

    pub async fn from_config(event_list: PubkyId) -> Result<EventsList, DynError> {
        // Attempt to load the event_list cursor from Redis
        match EventsList::get_from_index(&event_list).await? {
            Some(hs) => Ok(hs),
            None => {
                // Create a new EventsList instance with default cursor
                EventsList::new(event_list).await
            }
        }
    }
}
