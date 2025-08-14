use crate::db::execute_graph_operation;
use crate::db::get_neo4j_graph;
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

    /// Retrieves a homeserver from Neo4j.
    pub async fn get_from_graph(id: &str) -> Result<Option<Homeserver>, DynError> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query = queries::get::get_homeserver_by_id(id);

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        if let Some(row) = result.next().await? {
            let hs = Self {
                id: row.get("id").unwrap_or_default(),
                cursor: "0000000000000".to_string(), // TODO Should cursor also be stored in graph? If so, also updated? When?
            };
            Ok(Some(hs))
        } else {
            Ok(None)
        }
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
