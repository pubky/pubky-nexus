use crate::RedisOps;
use serde::{Deserialize, Serialize};

use super::user::PubkyId;

/// Represents a homeserver with its public key, URL, and cursor.
#[derive(Serialize, Deserialize, Debug)]
pub struct Homeserver {
    pub id: PubkyId,
    pub url: String, // Ideally we should not need URL for `/events` streams
    pub cursor: String,
}

impl RedisOps for Homeserver {}

impl Homeserver {
    /// Retrieves the homeserver from Redis.
    pub async fn get_from_index(
        id: &str,
    ) -> Result<Option<Self>, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(homeserver) = Self::try_from_index_json(&[id]).await? {
            return Ok(Some(homeserver));
        }
        Ok(None)
    }

    /// Stores the homeserver in Redis.
    pub async fn put_to_index(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.put_index_json(&[&self.id]).await?;
        Ok(())
    }
}
