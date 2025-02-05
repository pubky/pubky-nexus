use crate::db::graph::exec::retrieve_from_graph;
use crate::queries;
use crate::types::DynError;
use crate::{Config, RedisOps};
use async_trait::async_trait;
use chrono::Utc;
use neo4rs::Query;
use pubky_app_specs::PubkyId;
use serde::{Deserialize, Serialize};

use super::traits::Collection;

/// Represents a homeserver with its public key, URL, and cursor.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Homeserver {
    pub id: PubkyId,
    pub cursor: String,
    pub last_polled_at: i64,
    pub priority: u8,
    pub active: bool,
}

impl RedisOps for Homeserver {}

#[async_trait]
impl Collection<&str> for Homeserver {
    fn collection_details_graph_query(id_list: &[&str]) -> Query {
        queries::get::get_homeservers_details_by_ids(id_list)
    }

    fn put_graph_query(&self) -> Result<Query, DynError> {
        queries::put::create_homeserver(self)
    }

    async fn extend_on_index_miss(_: &[std::option::Option<Self>]) -> Result<(), DynError> {
        Ok(())
    }
}

impl Homeserver {
    pub fn new(id: PubkyId) -> Self {
        Homeserver {
            id,
            cursor: "0000000000000".to_string(),
            last_polled_at: Utc::now().timestamp_millis(),
            priority: 0,
            active: true,
        }
    }

    pub async fn save(&self) -> Result<(), DynError> {
        self.put_to_graph().await?;
        Homeserver::put_to_index(vec![self.id.as_str()].as_slice(), vec![Some(self.clone())])
            .await?;
        Ok(())
    }

    pub async fn from_config(config: &Config) -> Result<Homeserver, DynError> {
        let homeserver_id = config.homeserver.clone();
        // Create a PubkyId from the homeserver public key
        let id = PubkyId::try_from(&homeserver_id)?;

        let homeserver =
            Homeserver::get_by_ids(vec![homeserver_id.as_str()].as_slice()).await?[0].clone();

        // Attempt to load the homeserver cursor
        match homeserver {
            Some(hs) => Ok(hs),
            None => {
                // Create a new Homeserver instance with default cursor
                let hs = Homeserver::new(id);
                hs.save().await?;
                Ok(hs)
            }
        }
    }

    pub async fn get_next_homeservers(
        limit: i8,
        interval: u64,
    ) -> Result<Vec<Homeserver>, DynError> {
        let query = queries::get::get_next_homeservers(limit, interval);
        let result = retrieve_from_graph::<Vec<Homeserver>>(query, "homeservers").await?;
        match result {
            Some(homeservers) => Ok(homeservers),
            None => Ok(vec![]),
        }
    }
}
