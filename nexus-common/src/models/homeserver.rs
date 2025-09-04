use crate::db::exec_single_row;
use crate::db::fetch_key_from_graph;
use crate::db::queries;
use crate::db::RedisOps;
use crate::types::DynError;

use pubky_app_specs::PubkyId;
use serde::{Deserialize, Serialize};
use tracing::info;

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

    /// Mutates the cursor of the homeserver
    pub fn persist_cursor(&self, cursor: String) -> Self {
        Homeserver {
            id: self.id.clone(),
            cursor,
        }
    }

    /// Stores this homeserver in the graph.
    pub async fn put_to_graph(&self) -> Result<(), DynError> {
        let query = queries::put::create_homeserver(&self.id);
        exec_single_row(query).await
    }

    /// Retrieves a homeserver from Neo4j.
    ///
    /// Note that the cursor in the returned homeserver will have the default value, as it is not persisted in the graph.
    pub async fn get_from_graph(id: &str) -> Result<Option<Homeserver>, DynError> {
        let query = queries::get::get_homeserver_by_id(id);

        let maybe_id = fetch_key_from_graph(query, "id").await?;
        let maybe_hs = maybe_id.map(Homeserver::new);

        Ok(maybe_hs)
    }

    /// Retrieves the homeserver from Redis.
    pub async fn get_from_index(id: &str) -> Result<Option<Self>, DynError> {
        Self::try_from_index_json(&[id], None).await
    }

    /// Stores this homeserver in Redis.
    pub async fn put_to_index(&self) -> Result<(), DynError> {
        self.put_index_json(&[&self.id], None, None).await
    }

    pub async fn get_by_id(homeserver_id: PubkyId) -> Result<Option<Homeserver>, DynError> {
        match Homeserver::get_from_index(&homeserver_id).await? {
            Some(hs) => Ok(Some(hs)),
            None => match Self::get_from_graph(&homeserver_id).await? {
                Some(hs_grom_graph) => {
                    hs_grom_graph.put_to_index().await?;
                    Ok(Some(hs_grom_graph))
                }
                None => Ok(None),
            },
        }
    }

    /// Verifies if homeserver exists, or persists it if missing
    pub async fn persist_if_unknown(homeserver_id: PubkyId) -> Result<(), DynError> {
        if Self::get_by_id(homeserver_id.clone()).await?.is_none() {
            info!("Homeserver {} not found, persisting it", homeserver_id);
            let homeserver = Homeserver::new(homeserver_id);
            homeserver.put_to_graph().await?;
            homeserver.put_to_index().await?;
        }

        Ok(())
    }

    /// Retrieves all homeservers from the graph
    pub async fn get_all_from_graph() -> Result<Vec<String>, DynError> {
        let query = queries::get::get_all_homeservers();
        let homeservers: Option<Vec<String>> = fetch_key_from_graph(query, "homeservers_list").await?;
        if homeservers.is_none() {
            return Err("No homeservers found in graph".into());
        }
        Ok(homeservers.unwrap())
    }
}

#[cfg(test)]
mod tests {
    use pubky::Keypair;
    use pubky_app_specs::PubkyId;

    use crate::{types::DynError, StackConfig, StackManager};

    use super::*;

    #[tokio_shared_rt::test(shared)]
    async fn test_put_to_get_from_graph() -> Result<(), DynError> {
        StackManager::setup("unit-hs-test", &StackConfig::default()).await?;

        let keys = Keypair::random();
        let id = PubkyId::try_from(&keys.public_key().to_z32())?;

        let hs = Homeserver::new(id.clone());
        hs.put_to_graph()
            .await
            .expect("Failed to put homeserver to graph");

        let hs_from_graph = Homeserver::get_from_graph(&id)
            .await
            .ok()
            .flatten()
            .expect("Failed to get homeserver from graph");

        assert_eq!(id, hs_from_graph.id);

        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_put_to_get_from_index() -> Result<(), DynError> {
        StackManager::setup("unit-hs-test", &StackConfig::default()).await?;

        let keys = Keypair::random();
        let id = PubkyId::try_from(&keys.public_key().to_z32())?;

        let hs = Homeserver::new(id.clone());
        hs.put_to_index()
            .await
            .expect("Failed to put homeserver to index");

        let hs_from_index = Homeserver::get_from_index(&id)
            .await
            .ok()
            .flatten()
            .expect("Failed to get homeserver from index");

        assert_eq!(id, hs_from_index.id);

        Ok(())
    }
}
