use crate::db::exec_single_row;
use crate::db::fetch_key_from_graph;
use crate::db::kv::RedisError;
use crate::db::kv::RedisResult;
use crate::db::queries;
use crate::db::GraphError;
use crate::db::GraphResult;
use crate::db::{PubkyConnector, RedisOps};
use crate::models::error::ModelError;
use crate::models::error::ModelResult;
use crate::models::user::UserDetails;

use pubky::PublicKey;
use pubky_app_specs::ParsedUri;
use pubky_app_specs::PubkyId;
use serde::{Deserialize, Serialize};
use tracing::info;

/// Represents a homeserver with its public key, URL, and cursor.
#[derive(Serialize, Deserialize, Debug)]
pub struct Homeserver {
    pub id: PubkyId,

    // We persist this field only in the cache, but not in the graph.
    // Redis has regular snapshots, which ensures we get a recent state in case of RAM data loss (system crash).
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

    /// Creates a new homeserver instance with the specified cursor
    pub fn try_from_cursor<T: Into<String>>(id: PubkyId, cursor: T) -> ModelResult<Self> {
        let cursor = cursor.into();
        if cursor.is_empty() {
            return Err(ModelError::from_generic(
                "Cannot create a homeserver from an empty cursor",
            ));
        }

        Ok(Homeserver { id, cursor })
    }

    /// Stores this homeserver in the graph.
    pub async fn put_to_graph(&self) -> GraphResult<()> {
        let query = queries::put::create_homeserver(&self.id);
        exec_single_row(query).await
    }

    /// Retrieves a homeserver from Neo4j.
    ///
    /// Note that the cursor in the returned homeserver will have the default value, as it is not persisted in the graph.
    pub async fn get_from_graph(id: &str) -> GraphResult<Option<Homeserver>> {
        let query = queries::get::get_homeserver_by_id(id);

        let maybe_id = fetch_key_from_graph(query, "id").await?;
        let maybe_hs = maybe_id.map(Homeserver::new);

        Ok(maybe_hs)
    }

    /// Retrieves the homeserver from Redis.
    pub async fn get_from_index(id: &str) -> RedisResult<Option<Self>> {
        Self::try_from_index_json(&[id], None).await
    }

    /// Stores this homeserver in Redis.
    pub async fn put_to_index(&self) -> RedisResult<()> {
        if self.cursor.is_empty() {
            return Err(RedisError::InvalidInput(
                "Cannot save to index a homeserver with an empty cursor".into(),
            ));
        }
        self.put_index_json(&[&self.id], None, None).await
    }

    pub async fn get_by_id(homeserver_id: PubkyId) -> ModelResult<Option<Homeserver>> {
        match Homeserver::get_from_index(&homeserver_id).await? {
            Some(hs) => Ok(Some(hs)),
            None => match Self::get_from_graph(&homeserver_id).await? {
                Some(hs_from_graph) => {
                    hs_from_graph.put_to_index().await?;
                    Ok(Some(hs_from_graph))
                }
                None => Ok(None),
            },
        }
    }

    /// Verifies if homeserver exists in the graph, or persists it if missing
    pub async fn persist_if_unknown(homeserver_id: PubkyId) -> ModelResult<()> {
        if Self::get_from_graph(&homeserver_id).await?.is_none() {
            info!("Persisting new homeserver: {homeserver_id}");
            let homeserver = Homeserver::new(homeserver_id);
            homeserver.put_to_graph().await?;
            homeserver.put_to_index().await?;
        }

        Ok(())
    }

    /// Retrieves all homeservers from the graph.
    ///
    /// # Returns
    /// A list of all known homeserver IDs.
    ///
    /// # Errors
    /// Throws an error if no homeservers are found.
    pub async fn get_all_from_graph() -> GraphResult<Vec<String>> {
        let query = queries::get::get_all_homeservers();
        let maybe_hs_ids = fetch_key_from_graph(query, "homeservers_list").await?;
        let hs_ids: Vec<String> = maybe_hs_ids.unwrap_or_default();

        match hs_ids.is_empty() {
            true => Err(GraphError::Generic("No homeservers found in graph".into())),
            false => Ok(hs_ids),
        }
    }

    /// If a referenced post is hosted on a new, unknown homeserver, this method triggers ingestion of that homeserver.
    ///
    /// ### Arguments
    ///
    /// - `referenced_post_uri`: The parent post (if current post is a reply to it), or a reposted post (if current post is a Repost)
    pub async fn maybe_ingest_for_post(referenced_post_uri: &ParsedUri) -> ModelResult<()> {
        let ref_post_author_id = referenced_post_uri.user_id.as_str();

        Self::maybe_ingest_for_user(ref_post_author_id).await
    }

    /// If a referenced user is using a new, unknown homeserver, this method triggers ingestion of that homeserver.
    ///
    /// ### Arguments
    ///
    /// - `referenced_user_id`: The URI of the referenced user
    pub async fn maybe_ingest_for_user(referenced_user_id: &str) -> ModelResult<()> {
        let pubky = PubkyConnector::get().map_err(ModelError::from_generic)?;

        if UserDetails::get_by_id(referenced_user_id).await?.is_some() {
            tracing::debug!(
                "Skipping homeserver ingestion: author {referenced_user_id} already known"
            );
            return Ok(());
        }

        let ref_post_author_pk = referenced_user_id
            .parse::<PublicKey>()
            .map_err(ModelError::from_generic)?;
        let Some(ref_post_author_hs) = pubky.get_homeserver_of(&ref_post_author_pk).await else {
            tracing::warn!("Skipping homeserver ingestion: author {ref_post_author_pk} has no published homeserver");
            return Ok(());
        };

        // TODO This conversion can use from() instad of try_from() once pubky-app-specs bumps pubky to 0.7.x
        // let hs_pk = PubkyId::from(ref_post_author_hs.into_inner());
        let hs_pk = PubkyId::try_from(&ref_post_author_hs.into_inner().to_z32())
            .map_err(ModelError::from_generic)?;
        Self::persist_if_unknown(hs_pk.clone())
            .await
            .inspect(|_| tracing::info!("Ingested homeserver {hs_pk}"))
            .inspect_err(|e| tracing::error!("Failed to ingest homeserver {hs_pk}: {e}"))
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
