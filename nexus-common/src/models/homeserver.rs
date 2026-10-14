use crate::db::exec_single_row;
use crate::db::fetch_key_from_graph;
use crate::db::get_redis_conn;
use crate::db::kv::ensure_cursor_not_backwards;
use crate::db::kv::{cursor_backwards_error, RedisError, RedisResult};
use crate::db::queries;
use crate::db::GraphResult;
use crate::db::RedisOps;
use crate::models::error::ModelError;
use crate::models::error::ModelResult;
use deadpool_redis::redis::Script;
use pubky_app_specs::PubkyId;
use serde::{Deserialize, Deserializer, Serialize};
use std::sync::LazyLock;
use tracing::{info, warn};

/// Script for [`Homeserver::put_to_index`]. KEYS[1] is the homeserver's JSON key, ARGV[1] the
/// new cursor and ARGV[2] the homeserver JSON to store. Returns -1 without writing when the new
/// cursor is below the stored one, -2 when the stored cursor is not a number, else writes and
/// returns 1.
///
/// The cursors are compared as decimal strings rather than as Lua numbers, which are doubles
/// and lose precision past 2^53. The stored cursor is a JSON number, or a zero-padded string in
/// older entries (see `deserialize_cursor`).
static PUT_SCRIPT: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"local stored = redis.call('JSON.GET', KEYS[1], '$.cursor')
          if stored then
              local digits = string.match(stored, '^%["?(%d+)"?%]$')
              if not digits then
                  return -2
              end
              local old = string.gsub(digits, '^0+(%d)', '%1')
              local new = ARGV[1]
              if #new < #old or (#new == #old and new < old) then
                  return -1
              end
          end
          redis.call('JSON.SET', KEYS[1], '$', ARGV[2])
          return 1"#,
    )
});

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

    /// Creates a new homeserver instance with the specified cursor.
    ///
    /// Includes a validation call to [Self::validate_cursor_change],
    /// so that callers can detect invalid cursors early.
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

    /// Stores this homeserver in Redis, rejecting a cursor that would move backward.
    ///
    /// The compare and the write run as one script ([`PUT_SCRIPT`]), so writers racing on
    /// the same homeserver cannot rewind its cursor: whichever lands second, the lower value
    /// is rejected.
    pub async fn put_to_index(&self) -> RedisResult<()> {
        let payload = serde_json::to_string(self)
            .map_err(|e| RedisError::SerializationFailed(Box::new(e)))?;
        let key = format!("{}:{}", Self::prefix().await, self.id);

        let mut redis_conn = get_redis_conn().await?;
        let written: i64 = PUT_SCRIPT
            .key(key)
            .arg(self.cursor)
            .arg(payload)
            .invoke_async(&mut redis_conn)
            .await?;

        match written {
            1 => Ok(()),
            -1 => Err(cursor_backwards_error()),
            _ => Err(RedisError::DeserializationFailed(
                format!("stored cursor of homeserver {} is not a number", self.id).into(),
            )),
        }
    }

    /// Early check for [`Self::try_from_cursor`]. Not atomic with any write:
    /// [`Self::put_to_index`] repeats it in the same script as the write.
    async fn validate_cursor_change(id: &PubkyId, new_cursor: u64) -> RedisResult<()> {
        if let Some(existing) = Self::get_from_index(id).await? {
            ensure_cursor_not_backwards(new_cursor, existing.cursor)?;
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

    /// Returns all HS IDs with at least one active user, ordered by aggregate
    /// hosted trust descending, then by user count. Ties are in no particular
    /// order.
    ///
    /// Callers truncate this list, so the order decides which homeservers are
    /// polled at all. With no seed set configured nothing carries trust and the
    /// order falls through to user count, which is what it has always been.
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

    use crate::db::kv::RedisError;
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

    /// Two writers racing on one homeserver never rewind its cursor: whichever lands
    /// second, the lower value is rejected. A read-then-write lets both read the old
    /// value and the last write win.
    #[tokio_shared_rt::test(shared)]
    async fn test_racing_cursor_writes_never_rewind() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        for _ in 0..20 {
            let id = PubkyId::from(Keypair::random().public_key());
            let at = |cursor| Homeserver {
                id: id.clone(),
                cursor,
            };
            let (at_10, at_5) = (at(10), at(5));

            let (high, low) = tokio::join!(at_10.put_to_index(), at_5.put_to_index());

            high?;
            if let Err(err) = low {
                assert!(matches!(err, RedisError::InvalidInput(_)), "{err}");
            }
            let homeserver = Homeserver::get_from_index(&id)
                .await?
                .expect("homeserver should be in the index");
            assert_eq!(homeserver.cursor, 10);
        }

        Ok(())
    }

    /// The Redis key [`Homeserver::put_to_index`] writes `id` under.
    async fn index_key(id: &PubkyId) -> String {
        format!("{}:{id}", Homeserver::prefix().await)
    }

    /// Stores `json` as homeserver `id`'s entry with a raw `JSON.SET`, the way older
    /// writers or a corruption may have left it.
    async fn put_raw_entry(id: &PubkyId, json: &str) -> Result<(), DynError> {
        let mut redis_conn = get_redis_conn().await?;
        let _: () = deadpool_redis::redis::cmd("JSON.SET")
            .arg(index_key(id).await)
            .arg("$")
            .arg(json)
            .query_async(&mut redis_conn)
            .await?;
        Ok(())
    }

    /// Entries written before cursors were stored as numbers hold a zero-padded string,
    /// which the cursor check still reads and compares by value.
    #[tokio_shared_rt::test(shared)]
    async fn test_cursor_check_reads_a_zero_padded_string() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        let id = PubkyId::from(Keypair::random().public_key());
        put_raw_entry(&id, &format!(r#"{{"id":"{id}","cursor":"0000000000420"}}"#)).await?;

        let err = Homeserver {
            id: id.clone(),
            cursor: 100,
        }
        .put_to_index()
        .await
        .expect_err("a cursor below the stored 420 must be rejected");
        assert!(matches!(err, RedisError::InvalidInput(_)));

        for cursor in [420, 1_000] {
            Homeserver {
                id: id.clone(),
                cursor,
            }
            .put_to_index()
            .await?;
        }
        let homeserver = Homeserver::get_from_index(&id)
            .await?
            .expect("homeserver should be in the index");
        assert_eq!(homeserver.cursor, 1_000);

        Ok(())
    }

    /// The check compares cursors exactly past 2^53, where Lua numbers (doubles) would read
    /// 9007199254740992 and 9007199254740993 as the same value and let the rewind through.
    #[tokio_shared_rt::test(shared)]
    async fn test_cursor_check_is_exact_past_2_pow_53() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        let id = PubkyId::from(Keypair::random().public_key());
        let at = |cursor| Homeserver {
            id: id.clone(),
            cursor,
        };
        at(9_007_199_254_740_993).put_to_index().await?;

        let err = at(9_007_199_254_740_992)
            .put_to_index()
            .await
            .expect_err("a cursor one below the stored one must be rejected");
        assert!(matches!(err, RedisError::InvalidInput(_)), "{err}");
        let homeserver = Homeserver::get_from_index(&id)
            .await?
            .expect("homeserver should be in the index");
        assert_eq!(homeserver.cursor, 9_007_199_254_740_993);

        at(9_007_199_254_740_993).put_to_index().await?;

        Ok(())
    }

    /// A stored cursor that is not a number refuses the write, as it did when the check
    /// deserialized the entry, rather than being silently overwritten.
    #[tokio_shared_rt::test(shared)]
    async fn test_unreadable_stored_cursor_refuses_the_write() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        let id = PubkyId::from(Keypair::random().public_key());
        put_raw_entry(&id, &format!(r#"{{"id":"{id}","cursor":"abc"}}"#)).await?;
        let stored_entry = || async {
            let mut redis_conn = get_redis_conn().await?;
            let entry: String = deadpool_redis::redis::cmd("JSON.GET")
                .arg(index_key(&id).await)
                .query_async(&mut redis_conn)
                .await?;
            Ok::<_, DynError>(entry)
        };
        let before = stored_entry().await?;

        let err = Homeserver {
            id: id.clone(),
            cursor: 100,
        }
        .put_to_index()
        .await
        .expect_err("an unreadable stored cursor must refuse the write");

        assert!(matches!(err, RedisError::DeserializationFailed(_)), "{err}");
        assert_eq!(
            stored_entry().await?,
            before,
            "the entry must be left as it was"
        );

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
