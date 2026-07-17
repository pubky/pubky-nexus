use crate::db::exec_single_row;
use crate::db::fetch_key_from_graph;
use crate::db::kv::RedisError;
use crate::db::kv::RedisResult;
use crate::db::queries;
use crate::db::GraphResult;
use crate::db::RedisOps;
use crate::models::error::ModelError;
use crate::models::error::ModelResult;
use pubky_app_specs::PubkyId;
use serde::{Deserialize, Deserializer, Serialize};
use tracing::{info, warn};

/// A set of homeserver public keys forbidden from indexing and ingestion.
#[derive(Debug, Default, Clone)]
pub struct HsBlacklist(Vec<PubkyId>);

impl HsBlacklist {
    pub fn new(hs_pks: impl IntoIterator<Item = PubkyId>) -> Self {
        Self(hs_pks.into_iter().collect())
    }

    pub fn from_config(config: &crate::StackConfig) -> Self {
        Self::new(config.net.external_hs_pk_blacklist.iter().cloned())
    }

    pub fn is_blacklisted(&self, hs_id: &str) -> bool {
        self.0.iter().any(|pk| pk.as_ref() == hs_id)
    }
}

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
    #[serde(deserialize_with = "deserialize_cursor")]
    pub cursor: u64,
}

fn deserialize_cursor<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Cursor {
        Number(u64),
        String(String),
    }

    match Cursor::deserialize(deserializer)? {
        Cursor::Number(cursor) => Ok(cursor),
        Cursor::String(cursor) => cursor.parse().map_err(serde::de::Error::custom),
    }
}

impl RedisOps for Homeserver {}

impl Homeserver {
    /// Instantiates a new homeserver with default cursor
    pub fn new(id: PubkyId) -> Self {
        Homeserver { id, cursor: 0 }
    }

    /// Creates a new homeserver instance with the specified cursor
    pub async fn try_from_cursor<T: Into<String>>(id: PubkyId, cursor: T) -> ModelResult<Self> {
        let cursor = cursor.into();
        let cursor = cursor.parse().map_err(|_| {
            ModelError::from_generic(format!(
                "Cannot create a homeserver from a non-numeric cursor: {cursor}"
            ))
        })?;

        Self::validate_cursor_change(&id, cursor).await?;

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
        Self::validate_cursor_change(&self.id, self.cursor).await?;
        self.put_index_json(&[&self.id], None, None).await
    }

    async fn validate_cursor_change(id: &PubkyId, new_cursor: u64) -> RedisResult<()> {
        if let Some(existing) = Self::get_from_index(id).await? {
            if new_cursor < existing.cursor {
                return Err(RedisError::InvalidInput(
                    "Cursor cannot move backwards".into(),
                ));
            }
        }

        Ok(())
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

    /// Returns all HS IDs with at least one active user, sorted by user count descending.
    ///
    /// # Returns
    /// A list of active homeserver IDs.
    pub async fn get_all_active_from_graph() -> GraphResult<Vec<String>> {
        let query = queries::get::get_all_homeservers_with_active_users();
        let maybe_hs_ids = fetch_key_from_graph(query, "homeservers_list").await?;
        Ok(maybe_hs_ids.unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use pubky::Keypair;
    use pubky_app_specs::PubkyId;

    use crate::{types::DynError, StackConfig, StackManager};

    use super::*;

    #[test]
    fn test_deserialize_cursor_from_string() {
        let id = PubkyId::from(Keypair::random().public_key());
        for (cursor, expected) in [("\"0000000000000\"", 0), ("\"42\"", 42)] {
            let json = format!(r#"{{"id":"{id}","cursor":{cursor}}}"#);
            let homeserver: Homeserver = serde_json::from_str(&json).unwrap();

            assert_eq!(homeserver.cursor, expected);
        }
    }

    #[test]
    fn test_deserialize_cursor_from_number() {
        let id = PubkyId::from(Keypair::random().public_key());
        for (cursor, expected) in [("0", 0), ("1234567890123", 1_234_567_890_123)] {
            let json = format!(r#"{{"id":"{id}","cursor":{cursor}}}"#);
            let homeserver: Homeserver = serde_json::from_str(&json).unwrap();

            assert_eq!(homeserver.cursor, expected);
        }
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_put_to_get_from_graph() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        let keys = Keypair::random();
        let id = PubkyId::from(keys.public_key());

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
        let id = PubkyId::from(keys.public_key());

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
    async fn test_cursor_forwards_accepted() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        let id = PubkyId::from(Keypair::random().public_key());
        Homeserver::try_from_cursor(id.clone(), "100")
            .await?
            .put_to_index()
            .await?;
        Homeserver::try_from_cursor(id.clone(), "200")
            .await?
            .put_to_index()
            .await?;

        let homeserver = Homeserver::get_from_index(&id)
            .await?
            .expect("homeserver should be in the index");
        assert_eq!(homeserver.cursor, 200);

        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_cursor_backwards_rejected_by_put_to_index() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        let id = PubkyId::from(Keypair::random().public_key());
        Homeserver::try_from_cursor(id.clone(), "500")
            .await?
            .put_to_index()
            .await?;

        let err = Homeserver {
            id: id.clone(),
            cursor: 100,
        }
        .put_to_index()
        .await
        .expect_err("backward cursor must be rejected");
        assert!(matches!(err, RedisError::InvalidInput(_)));

        let homeserver = Homeserver::get_from_index(&id)
            .await?
            .expect("homeserver should remain in the index");
        assert_eq!(homeserver.cursor, 500);

        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_cursor_backwards_rejected_by_try_from_cursor() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        let id = PubkyId::from(Keypair::random().public_key());
        Homeserver::try_from_cursor(id.clone(), "300")
            .await?
            .put_to_index()
            .await?;

        let err = Homeserver::try_from_cursor(id.clone(), "50")
            .await
            .expect_err("backward cursor must be rejected");
        assert!(matches!(
            err,
            ModelError::KvOperationFailed(RedisError::InvalidInput(_))
        ));

        let homeserver = Homeserver::try_from_cursor(id, "400").await?;
        assert_eq!(homeserver.cursor, 400);

        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_cursor_equal_value_accepted() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        let id = PubkyId::from(Keypair::random().public_key());
        Homeserver::try_from_cursor(id.clone(), "200")
            .await?
            .put_to_index()
            .await?;
        Homeserver::try_from_cursor(id.clone(), "200")
            .await?
            .put_to_index()
            .await?;

        let homeserver = Homeserver::get_from_index(&id)
            .await?
            .expect("homeserver should be in the index");
        assert_eq!(homeserver.cursor, 200);

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
        assert_eq!(from_index.cursor, 0);
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
        Homeserver::try_from_cursor(id.clone(), "1234567890123")
            .await?
            .put_to_index()
            .await?;

        Homeserver::persist_if_unknown(id.clone()).await?;

        let from_index = Homeserver::get_from_index(&id)
            .await?
            .expect("index entry should still be present");
        assert_eq!(
            from_index.cursor, 1_234_567_890_123,
            "existing cursor must not be overwritten"
        );
        Ok(())
    }
}
