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

use pubky_app_specs::ParsedUri;
use pubky_app_specs::PubkyId;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// Represents a homeserver with its public key, URL, and cursor.
#[derive(Serialize, Deserialize, Debug)]
pub struct Homeserver {
    pub id: PubkyId,

    // Cursor lives in Redis only (not the graph). Redis snapshots can
    // lag, so a crash may lose recent advancement — the watcher then
    // resumes from whatever cursor survived in the snapshot and catches
    // up from the homeserver. We do NOT re-index from scratch on partial
    // loss; `persist_if_unknown` only re-seeds cursor=0 when the
    // Homeserver key is missing from Redis entirely (e.g. wiped volume),
    // never on a routine crash.
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
        // No graph fallback. A cache miss (or a Redis read error silently
        // surfaced as `Ok(None)` by `json::get`) is treated as a failure
        // and propagated to the caller, which puts the homeserver into
        // backoff. The previous behaviour rebuilt from the graph (which
        // does not store the cursor) and wrote cursor=0 back to Redis,
        // silently overwriting the real value on a transient Redis hiccup.
        Ok(Self::get_from_index(&homeserver_id).await?)
    }

    /// Ensures the homeserver is recorded in both the graph and the index.
    ///
    /// - First-time install (missing from both): writes a fresh
    ///   [`Homeserver::new`] to graph and index.
    /// - Asymmetric state (present in graph, missing from index): re-seeds the
    ///   index with the default cursor and logs a warning. This is the
    ///   self-heal path for a wiped Redis volume sitting alongside a persisted
    ///   Neo4j volume — common in dev/testnet workflows where operators tear
    ///   down one store but not the other. The watcher will then re-index
    ///   from `cursor=0`, which is the same recovery the system performs
    ///   after a Redis crash by design.
    /// - Both already present: no-op.
    ///
    /// Safe to call repeatedly. Relies on `get_from_index` returning a
    /// reliable `Ok(None)` for genuine cache misses; if the read errors, the
    /// error propagates and the re-seed never fires.
    pub async fn persist_if_unknown(homeserver_id: PubkyId) -> ModelResult<()> {
        let in_graph = Self::get_from_graph(&homeserver_id).await?.is_some();
        let in_index = Self::get_from_index(&homeserver_id).await?.is_some();

        if in_graph && in_index {
            return Ok(());
        }

        let homeserver = Homeserver::new(homeserver_id.clone());

        if !in_graph {
            info!("Persisting new homeserver to graph: {homeserver_id}");
            homeserver.put_to_graph().await?;
        }

        if !in_index {
            if in_graph {
                warn!(
                    "Homeserver {homeserver_id} present in graph but missing from index; \
                     re-seeding index with default cursor — watcher will re-index from 0"
                );
            } else {
                info!("Persisting new homeserver to index: {homeserver_id}");
            }
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
        Self::maybe_ingest_for_user(&referenced_post_uri.user_id).await
    }

    /// If a referenced user is using a new, unknown homeserver, this method triggers ingestion of that homeserver.
    ///
    /// ### Arguments
    ///
    /// - `referenced_user_id`: The `PubkyId` of the referenced user
    #[tracing::instrument(name = "homeserver.ingest", skip_all)]
    pub async fn maybe_ingest_for_user(referenced_user_id: &PubkyId) -> ModelResult<()> {
        let pubky = PubkyConnector::get().map_err(ModelError::from_generic)?;

        if UserDetails::get_by_id(referenced_user_id.as_ref())
            .await?
            .is_some()
        {
            tracing::debug!(
                "Skipping homeserver ingestion: author {referenced_user_id} already known"
            );
            return Ok(());
        }

        let ref_post_author_pk = referenced_user_id.to_public_key();
        let Some(ref_post_author_hs) = pubky.get_homeserver_of(&ref_post_author_pk).await else {
            tracing::warn!("Skipping homeserver ingestion: author {ref_post_author_pk} has no published homeserver");
            return Ok(());
        };

        let hs_pk = PubkyId::from(ref_post_author_hs);
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
        StackManager::setup(&StackConfig::default()).await?;

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
        StackManager::setup(&StackConfig::default()).await?;

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

    #[tokio_shared_rt::test(shared)]
    async fn test_persist_if_unknown_first_time_install_writes_both() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        let keys = Keypair::random();
        let id = PubkyId::try_from(&keys.public_key().to_z32())?;

        Homeserver::persist_if_unknown(id.clone()).await?;

        assert!(Homeserver::get_from_graph(&id).await?.is_some());
        assert!(Homeserver::get_from_index(&id).await?.is_some());
        Ok(())
    }

    // Regression test for the docker-testnet asymmetric-storage scenario:
    // graph volume persisted, Redis volume wiped. Before this fix, watcher
    // looped on "Homeserver not found" forever because get_by_id only reads
    // the index and persist_if_unknown only wrote when the graph was empty.
    #[tokio_shared_rt::test(shared)]
    async fn test_persist_if_unknown_reseeds_index_when_only_graph_has_it() -> Result<(), DynError>
    {
        StackManager::setup(&StackConfig::default()).await?;

        let keys = Keypair::random();
        let id = PubkyId::try_from(&keys.public_key().to_z32())?;

        // Simulate "graph persisted, Redis wiped": write to graph only.
        Homeserver::new(id.clone()).put_to_graph().await?;
        assert!(
            Homeserver::get_from_index(&id).await?.is_none(),
            "precondition: index should be empty for a fresh keypair"
        );

        Homeserver::persist_if_unknown(id.clone()).await?;

        let from_index = Homeserver::get_from_index(&id)
            .await?
            .expect("index should be re-seeded after persist_if_unknown");
        assert_eq!(from_index.id, id);
        assert_eq!(from_index.cursor, "0000000000000");
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_persist_if_unknown_is_noop_when_both_present() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        let keys = Keypair::random();
        let id = PubkyId::try_from(&keys.public_key().to_z32())?;

        // Seed both stores with a non-default cursor to prove the no-op
        // branch doesn't overwrite an existing index entry.
        Homeserver::new(id.clone()).put_to_graph().await?;
        Homeserver::try_from_cursor(id.clone(), "1234567890123")?
            .put_to_index()
            .await?;

        Homeserver::persist_if_unknown(id.clone()).await?;

        let from_index = Homeserver::get_from_index(&id)
            .await?
            .expect("index entry should still be present");
        assert_eq!(
            from_index.cursor, "1234567890123",
            "existing cursor must not be overwritten"
        );
        Ok(())
    }
}
