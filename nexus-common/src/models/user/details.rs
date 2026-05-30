use super::UserSearch;
use crate::db::graph::Query;
use crate::db::kv::RedisResult;
use crate::db::{exec_single_row, queries, GraphResult, PubkyConnector, RedisOps};
use crate::models::error::{ModelError, ModelResult};
use crate::models::traits::Collection;
use async_trait::async_trait;
use chrono::Utc;
use pubky::PublicKey;
use pubky_app_specs::{PubkyAppUser, PubkyAppUserLink, PubkyId};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json;
use utoipa::ToSchema;

const USER_HS_CURSOR: [&str; 2] = ["Users", "Homeservers"];

/// Redis key parts for per-user homeserver cursor storage: `["Users", "Homeservers", <user_id>]`.
pub type UserHsCursorKey<'a> = [&'a str; 3];

/// Builds the Redis key path for per-user homeserver cursor storage
pub fn user_hs_cursor_key(user_id: &str) -> UserHsCursorKey<'_> {
    [USER_HS_CURSOR[0], USER_HS_CURSOR[1], user_id]
}

#[async_trait]
impl RedisOps for UserDetails {}

#[async_trait]
impl Collection<&str> for UserDetails {
    fn collection_details_graph_query(id_list: &[&str]) -> Query {
        queries::get::get_users_details_by_ids(id_list)
    }

    fn put_graph_query(&self) -> GraphResult<Query> {
        queries::put::create_user(self)
    }

    async fn extend_on_index_miss(details: &[std::option::Option<Self>]) -> RedisResult<()> {
        let user_details_refs: Vec<&UserDetails> = details
            .iter()
            .filter_map(|detail| detail.as_ref())
            .collect();

        UserSearch::put_to_index(&user_details_refs).await
    }
}

/// Represents user data with name, bio, image, links, and status.
#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct UserDetails {
    pub name: String,
    pub bio: Option<String>,
    pub id: PubkyId,
    #[serde(deserialize_with = "deserialize_user_links")]
    pub links: Option<Vec<PubkyAppUserLink>>,
    pub status: Option<String>,
    pub image: Option<String>,
    pub indexed_at: i64,
}

fn deserialize_user_links<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<PubkyAppUserLink>>, D::Error>
where
    D: Deserializer<'de>,
{
    // Deserialize as Option to handle missing properties in neo4rs.
    // Neo4j drops null properties from nodes, so when a node lacks the links
    // property, neo4rs provides a fallback deserializer that only handles
    // deserialize_option (returning None), not deserialize_any.
    let value = match Option::<serde_json::Value>::deserialize(deserializer)? {
        Some(v) => v,
        None => return Ok(None),
    };

    match value {
        serde_json::Value::String(s) => {
            // If it's a string, parse the string as JSON
            let urls: Option<Vec<PubkyAppUserLink>> =
                serde_json::from_str(&s).map_err(serde::de::Error::custom)?;
            Ok(urls)
        }
        serde_json::Value::Array(arr) => {
            // If it's already an array, deserialize it directly
            let urls: Vec<PubkyAppUserLink> = serde_json::from_value(serde_json::Value::Array(arr))
                .map_err(serde::de::Error::custom)?;
            Ok(Some(urls))
        }
        serde_json::Value::Null => Ok(None),
        _ => Err(serde::de::Error::custom(
            "Expected either a string, an array or null",
        )),
    }
}

impl UserDetails {
    /// Retrieves details by user ID, first trying to get from Redis, then from Neo4j if not found.
    pub async fn get_by_id(user_id: &str) -> ModelResult<Option<Self>> {
        // Delegate to UserDetailsCollection::get_by_ids for single item retrieval
        let details_collection = Self::get_by_ids(&[user_id]).await?;
        Ok(details_collection.into_iter().flatten().next())
    }

    /// Creates a minimal `UserDetails` with only the public key.
    /// All profile fields (bio, links, status, image) default to `None`.
    pub fn from_pubky(user_id: PubkyId) -> Self {
        UserDetails {
            name: user_id.to_string(),
            id: user_id.clone(),
            indexed_at: Utc::now().timestamp_millis(),
            bio: None,
            links: None,
            status: None,
            image: None,
        }
    }

    pub fn from_homeserver(homeserver_user: PubkyAppUser, user_id: &PubkyId) -> Self {
        UserDetails {
            name: homeserver_user.name,
            bio: homeserver_user.bio,
            status: homeserver_user.status,
            links: homeserver_user.links,
            image: homeserver_user.image,
            id: user_id.clone(),
            indexed_at: Utc::now().timestamp_millis(),
        }
    }

    pub async fn delete(user_id: &str) -> ModelResult<()> {
        // Delete user_details on Redis
        Self::remove_from_index_multiple_json(&[&[user_id]]).await?;
        // Delete user graph node;
        exec_single_row(queries::del::delete_user(user_id)).await?;

        Ok(())
    }

    /// If a referenced user is unknown, not ingested in the graph yet, resolves their homeserver
    /// and persists the user node in the graph.
    #[tracing::instrument(name = "user.ingest", skip_all)]
    pub async fn maybe_ingest_user(user_id: &str) -> ModelResult<()> {
        if Self::get_by_id(user_id).await?.is_some() {
            tracing::debug!("Skipping user ingestion: {user_id} already known");
            return Ok(());
        }

        let pubky = PubkyConnector::get().map_err(ModelError::from_generic)?;

        let user_pk = user_id
            .parse::<PublicKey>()
            .map_err(ModelError::from_generic)?;

        let Some(hs_pk) = pubky.get_homeserver_of(&user_pk).await else {
            tracing::warn!(
                "Skipping user ingestion: {user_id} has no published homeserver or it's a homeserver pubky"
            );
            return Ok(());
        };

        let pubky_id = PubkyId::from(user_pk);
        let user_details = Self::from_pubky(pubky_id);

        let hs_id = &hs_pk.into_inner().to_z32();

        // Do not add to index, as this would affect the timeline of events for this user.
        // Only create stub graph node for HS-resolver to store user-HS mapping.
        user_details
            .put_to_graph()
            .await
            .inspect(|_| tracing::info!("Ingested user {user_id} from homeserver {hs_id}"))
            .inspect_err(|e| tracing::error!("Failed to ingest user {user_id}: {e}"))?;

        // Store the start point of the homeserver cursor
        Self::write_hs_cursor(user_id, hs_id, 0).await?;

        Ok(())
    }

    /// Batch-reads each user's stored event cursor for `hs_id`, returning `0`
    /// for users with no cursor entry yet (newly ingested).
    ///
    /// Each user's cursor lives in its own `USER_HS_CURSOR` sorted set (keyed by
    /// user ID) with the homeserver ID as the member; all lookups are batched
    /// into a single `check_sorted_set_members` pipeline call. Redis errors are
    /// propagated instead of silently rewinding to 0.
    ///
    /// The cursor is stored as the score (f64), exact for integer values up to
    /// 2^53 — practically unreachable for monotonic event IDs.
    pub async fn read_hs_cursors(user_ids: &[&str], hs_id: &str) -> RedisResult<Vec<u64>> {
        let keys: Vec<UserHsCursorKey> = user_ids.iter().map(|u| user_hs_cursor_key(u)).collect();
        let pairs: Vec<(&[&str], &[&str])> = keys
            .iter()
            .map(|k| (k.as_slice(), std::slice::from_ref(&hs_id)))
            .collect();
        let scores = Self::check_sorted_set_members(None, &pairs).await?;
        Ok(scores.into_iter().map(|s| s.unwrap_or(0) as u64).collect())
    }

    /// Persists a single user's event cursor for `hs_id` to its `USER_HS_CURSOR` sorted set.
    pub async fn write_hs_cursor(user_id: &str, hs_id: &str, cursor: u64) -> RedisResult<()> {
        let key = user_hs_cursor_key(user_id);
        Self::put_index_sorted_set(&key, &[(cursor as f64, hs_id)], None, None).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neo4rs::{BoltInteger, BoltList, BoltMap, BoltNode, BoltString, BoltType, Node};
    use pubky::Keypair;

    use crate::{types::DynError, StackConfig, StackManager};

    /// Deserializing a UserDetails from a BoltNode without the links property
    /// should succeed with links: None. Neo4j drops null properties from nodes,
    /// so this is the expected shape after a roundtrip with links: None.
    #[test]
    fn deserialize_from_node_without_links() {
        let mut props = BoltMap::new();
        props.put(BoltString::from("name"), BoltType::from("Dave"));
        props.put(
            BoltString::from("id"),
            // Use valid PubkyId of test moderation user
            BoltType::from("uo7jgkykft4885n8cruizwy6khw71mnu5pq3ay9i8pw1ymcn85ko"),
        );
        props.put(
            BoltString::from("indexed_at"),
            BoltType::from(1724134095000_i64),
        );

        let node = Node::new(BoltNode::new(
            BoltInteger::new(1),
            BoltList::from(vec![BoltType::from("User")]),
            props,
        ));

        let details: UserDetails = node
            .to()
            .expect("should deserialize without links property (Neo4j drops null properties)");
        assert_eq!(details.name, "Dave");
        assert!(details.links.is_none());
    }

    /// `write_hs_cursor` persists a per-user cursor that `read_hs_cursors` reads
    /// back, missing entries default to 0, and re-writing overwrites the value.
    #[tokio_shared_rt::test(shared)]
    async fn test_hs_cursor_read_write_roundtrip() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        // Random IDs keep the test isolated from mock data and other tests.
        let user_with_cursor = PubkyId::from(Keypair::random().public_key()).to_string();
        let user_without_cursor = PubkyId::from(Keypair::random().public_key()).to_string();
        let hs_id = PubkyId::from(Keypair::random().public_key()).to_string();

        // A user with no stored cursor reads back as 0.
        let cursors = UserDetails::read_hs_cursors(&[user_with_cursor.as_str()], &hs_id).await?;
        assert_eq!(cursors, vec![0]);

        // A written cursor round-trips; a user without an entry stays at 0.
        UserDetails::write_hs_cursor(&user_with_cursor, &hs_id, 42).await?;
        let cursors = UserDetails::read_hs_cursors(
            &[user_with_cursor.as_str(), user_without_cursor.as_str()],
            &hs_id,
        )
        .await?;
        assert_eq!(cursors, vec![42, 0]);

        // Writing again overwrites the previous value.
        UserDetails::write_hs_cursor(&user_with_cursor, &hs_id, 100).await?;
        let cursors = UserDetails::read_hs_cursors(&[user_with_cursor.as_str()], &hs_id).await?;
        assert_eq!(cursors, vec![100]);

        Ok(())
    }
}
