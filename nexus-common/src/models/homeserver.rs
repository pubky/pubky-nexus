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
use serde::{Deserialize, Deserializer, Serialize};
use tracing::info;

/// Deserializes cursor from either a JSON string or number.
///
/// This handles backwards compatibility with old data where cursor was stored as a string
/// (e.g., `"0000000000000"`), while also supporting the new numeric format.
fn deserialize_cursor<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum CursorValue {
        Number(u64),
        String(String),
    }

    match CursorValue::deserialize(deserializer)? {
        CursorValue::Number(n) => Ok(n),
        CursorValue::String(s) => s
            .parse()
            .map_err(|_| D::Error::custom(format!("Cannot parse cursor string '{s}' as u64"))),
    }
}

/// Represents a homeserver with its public key, URL, and cursor.
#[derive(Serialize, Deserialize, Debug)]
pub struct Homeserver {
    pub id: PubkyId,

    // We persist this field only in the cache, but not in the graph.
    // Redis has regular snapshots, which ensures we get a recent state in case of RAM data loss (system crash).
    #[serde(deserialize_with = "deserialize_cursor")]
    pub cursor: u64,
}

impl RedisOps for Homeserver {}

impl Homeserver {
    /// Instantiates a new homeserver with default cursor
    pub fn new(id: PubkyId) -> Self {
        Homeserver { id, cursor: 0 }
    }

    /// Creates a new homeserver instance with the specified cursor
    pub async fn try_from_cursor<T: Into<String>>(id: PubkyId, cursor: T) -> ModelResult<Self> {
        let cursor_str = cursor.into();
        let cursor = cursor_str.parse().map_err(|_| {
            ModelError::from_generic(format!(
                "Cannot create a HS from a non-numeric cursor: {cursor_str}"
            ))
        })?;

        Self::validate_cursor_change(&id, cursor).await?;

        Ok(Homeserver { id, cursor })
    }

    /// Stores this homeserver in the graph.
    pub async fn put_to_graph(&self) -> ModelResult<()> {
        let query = queries::put::create_homeserver(&self.id);
        exec_single_row(query).await.map_err(Into::into)
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

    pub async fn get_by_id(homeserver_id: PubkyId) -> ModelResult<Option<Homeserver>> {
        match Homeserver::get_from_index(&homeserver_id).await? {
            Some(hs) => Ok(Some(hs)),
            None => match Self::get_from_graph(&homeserver_id).await? {
                Some(hs_from_graph) => {
                    // This assumes the index and the graph are in-sync
                    // If they are not (e.g. Redis lost a HS entry but graph still has it), put_to_index will persist with cursor = 0
                    hs_from_graph.put_to_index().await?;
                    Ok(Some(hs_from_graph))
                }
                None => Ok(None),
            },
        }
    }

    async fn validate_cursor_change(id: &str, new_cursor: u64) -> RedisResult<()> {
        // If we already indexed a value, reject cursors going below it to prevent reindexing past events
        if let Some(hs_from_index) = Self::get_from_index(id).await? {
            if new_cursor < hs_from_index.cursor {
                return Err(RedisError::InvalidInput(
                    "Cursor cannot move backwards".into(),
                ));
            }
        }

        Ok(())
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

        let hs_pk = PubkyId::from(ref_post_author_hs.into_inner());
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

    #[tokio_shared_rt::test(shared)]
    async fn test_cursor_forwards_accepted() -> Result<(), DynError> {
        StackManager::setup("unit-hs-test", &StackConfig::default()).await?;

        let keys = Keypair::random();
        let id = PubkyId::try_from(&keys.public_key().to_z32())?;

        // Store cursor at 100
        let hs = Homeserver {
            id: id.clone(),
            cursor: 100,
        };
        hs.put_to_index()
            .await
            .expect("Failed to put initial cursor");

        // Moving cursor forward to 200 must succeed
        let hs2 = Homeserver {
            id: id.clone(),
            cursor: 200,
        };
        hs2.put_to_index()
            .await
            .expect("Forward cursor update should be accepted");

        let stored = Homeserver::get_from_index(&id)
            .await
            .unwrap()
            .expect("Homeserver not found in index");
        assert_eq!(stored.cursor, 200);

        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_cursor_backwards_rejected_by_put_to_index() -> Result<(), DynError> {
        StackManager::setup("unit-hs-test", &StackConfig::default()).await?;

        let keys = Keypair::random();
        let id = PubkyId::try_from(&keys.public_key().to_z32())?;

        // Store cursor at 500
        let hs = Homeserver {
            id: id.clone(),
            cursor: 500,
        };
        hs.put_to_index()
            .await
            .expect("Failed to put initial cursor");

        // Attempting to move cursor backwards to 100 must be rejected
        let hs2 = Homeserver {
            id: id.clone(),
            cursor: 100,
        };
        let result = hs2.put_to_index().await;
        assert!(
            result.is_err(),
            "Backwards cursor update should be rejected"
        );

        // The stored cursor must remain at 500
        let stored = Homeserver::get_from_index(&id)
            .await
            .unwrap()
            .expect("Homeserver not found in index");
        assert_eq!(stored.cursor, 500);

        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_cursor_backwards_rejected_by_try_from_cursor() -> Result<(), DynError> {
        StackManager::setup("unit-hs-test", &StackConfig::default()).await?;

        let keys = Keypair::random();
        let id = PubkyId::try_from(&keys.public_key().to_z32())?;

        // Store cursor at 300
        let hs = Homeserver {
            id: id.clone(),
            cursor: 300,
        };
        hs.put_to_index()
            .await
            .expect("Failed to put initial cursor");

        // try_from_cursor with a lower value must fail
        let result = Homeserver::try_from_cursor(id.clone(), "50").await;
        assert!(
            result.is_err(),
            "try_from_cursor with backwards cursor should be rejected"
        );

        // try_from_cursor with a higher value must succeed
        let result = Homeserver::try_from_cursor(id.clone(), "400").await;
        assert!(
            result.is_ok(),
            "try_from_cursor with forward cursor should be accepted"
        );
        assert_eq!(result.unwrap().cursor, 400);

        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_cursor_equal_value_accepted() -> Result<(), DynError> {
        StackManager::setup("unit-hs-test", &StackConfig::default()).await?;

        let keys = Keypair::random();
        let id = PubkyId::try_from(&keys.public_key().to_z32())?;

        // Store cursor at 200
        let hs = Homeserver {
            id: id.clone(),
            cursor: 200,
        };
        hs.put_to_index()
            .await
            .expect("Failed to put initial cursor");

        // Writing the same cursor value (not backwards) must succeed
        let hs2 = Homeserver {
            id: id.clone(),
            cursor: 200,
        };
        hs2.put_to_index()
            .await
            .expect("Same cursor value should be accepted");

        Ok(())
    }

    #[test]
    fn test_deserialize_cursor_from_string() {
        // Simulates old data format where cursor was stored as a string
        let json = r#"{"id":"o1gg96ewuojmopc9qcp6j3kk5rn1b81ks6hisk7jitpptgeo3dty","cursor":"0000000000000"}"#;
        let hs: Homeserver = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(hs.cursor, 0);

        // Also test with a non-zero string cursor
        let json =
            r#"{"id":"o1gg96ewuojmopc9qcp6j3kk5rn1b81ks6hisk7jitpptgeo3dty","cursor":"12345"}"#;
        let hs: Homeserver = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(hs.cursor, 12345);
    }

    #[test]
    fn test_deserialize_cursor_from_number() {
        // Current format where cursor is stored as a number
        let json = r#"{"id":"o1gg96ewuojmopc9qcp6j3kk5rn1b81ks6hisk7jitpptgeo3dty","cursor":0}"#;
        let hs: Homeserver = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(hs.cursor, 0);

        // Also test with a non-zero numeric cursor
        let json =
            r#"{"id":"o1gg96ewuojmopc9qcp6j3kk5rn1b81ks6hisk7jitpptgeo3dty","cursor":98765}"#;
        let hs: Homeserver = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(hs.cursor, 98765);
    }
}
