use crate::types::DynError;
use crate::types::PubkyId;
use crate::{Config, RedisOps};
use serde::{Deserialize, Serialize};

/// Represents a homeserver with its public key, URL, and cursor.
#[derive(Serialize, Deserialize, Debug)]
pub struct Homeserver {
    pub id: PubkyId,
    pub cursor: String,
}

impl RedisOps for Homeserver {}

impl Homeserver {
    pub async fn new(id: PubkyId) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        let hs = Homeserver {
            id,
            cursor: "0000000000000".to_string(),
        };
        // Store homeserver with initial cursor in Index
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

    /// Stores the homeserver in Redis.
    pub async fn put_to_index(&self) -> Result<(), DynError> {
        self.put_index_json(&[&self.id], None, None).await?;
        Ok(())
    }

    pub async fn from_config(config: &Config) -> Result<Homeserver, DynError> {
        let homeserver_id = config.homeserver.clone();
        // Create a PubkyId from the homeserver public key
        let id = PubkyId::try_from(&homeserver_id)?;

        // Attempt to load the homeserver cursor from Redis
        match Homeserver::get_from_index(&id).await? {
            Some(hs) => Ok(hs),
            None => {
                // Create a new Homeserver instance with default cursor
                Homeserver::new(id).await
            }
        }
    }
}
