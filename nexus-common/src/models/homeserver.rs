use crate::db::execute_graph_operation;
use crate::db::queries;
use crate::db::OperationOutcome;
use crate::db::RedisOps;
use crate::types::DynError;

use chrono::Utc;
use pubky_app_specs::PubkyId;
use serde::{Deserialize, Serialize};

/// Represents a homeserver with its public key, URL, and cursor.
#[derive(Serialize, Deserialize, Debug)]
pub struct Homeserver {
    pub id: PubkyId,
    pub cursor: String,
}

impl RedisOps for Homeserver {}

impl Homeserver {
    /// Instantiates a new homeserver with default cursor
    pub fn new(id: PubkyId) -> Self {
        Homeserver {
            id,
            cursor: "0000000000000".to_string(),
        }
    }

    /// Stores this homeserver in the graph.
    pub async fn put_to_graph(&self) -> Result<OperationOutcome, DynError> {
        let indexed_at = Utc::now().timestamp_millis();
        let query = queries::put::create_homeserver(&self.id, indexed_at);
        execute_graph_operation(query).await
    }

    /// Retrieves the homeserver from Redis.
    pub async fn get_from_index(id: &str) -> Result<Option<Self>, DynError> {
        Self::try_from_index_json(&[id], None).await
    }

    /// Stores the homeserver in Redis.
    pub async fn put_to_index(&self) -> Result<(), DynError> {
        self.put_index_json(&[&self.id], None, None).await
    }

    pub async fn from_config(homeserver: PubkyId) -> Result<Homeserver, DynError> {
        // Attempt to load the homeserver cursor from Redis
        match Homeserver::get_from_index(&homeserver).await? {
            Some(hs) => Ok(hs),
            None => {
                // Create a new Homeserver instance with default cursor
                Homeserver::new(homeserver).await
            }
        }
    }
}
