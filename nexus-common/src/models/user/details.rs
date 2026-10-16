use super::Epoch;
use crate::db::{
    exec_single_row, fetch_all_rows_from_graph, fetch_key_from_graph, queries, GraphResult,
};
use crate::models::error::ModelResult;
use chrono::Utc;
use pubky_app_specs::{PubkyAppUser, PubkyAppUserLink, PubkyId};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json;
use utoipa::ToSchema;

/// Represents user data with name, bio, image, links, and status. Read from
/// the graph, never cached.
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
    #[serde(deserialize_with = "deserialize_user_deleted", default)]
    pub deleted: bool,
    #[serde(deserialize_with = "deserialize_user_epoch", default)]
    pub epoch: Epoch,
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

fn deserialize_user_deleted<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    // Neo4j drops null/missing properties, so pre-migration nodes will lack the
    // `deleted` field entirely. Deserialize through Option<bool> so a missing
    // property yields None (→ false), while a present value is respected.
    // Serializes as a plain non-optional `bool` for the API.
    let value = Option::<bool>::deserialize(deserializer)?;
    Ok(value.unwrap_or(false))
}

fn deserialize_user_epoch<'de, D>(deserializer: D) -> Result<Epoch, D::Error>
where
    D: Deserializer<'de>,
{
    // Nodes without an epoch lack the field. neo4rs hands a missing node
    // property to a deserializer that only accepts `Option`, so
    // `#[serde(default)]` alone is not enough.
    let value = Option::<Epoch>::deserialize(deserializer)?;
    Ok(value.unwrap_or_default())
}

impl UserDetails {
    /// Reads the user's details from the graph, `None` when there is no such node.
    pub async fn get_by_id(user_id: &str) -> ModelResult<Option<Self>> {
        Ok(Self::get_by_ids(&[user_id]).await?.pop().flatten())
    }

    /// Reads each user's details from the graph: one slot per id, in order,
    /// `None` where there is no such node.
    pub async fn get_by_ids(user_ids: &[impl AsRef<str>]) -> ModelResult<Vec<Option<Self>>> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<&str> = user_ids.iter().map(AsRef::as_ref).collect();
        let rows = fetch_all_rows_from_graph(queries::get::get_users_details_by_ids(&ids)).await?;
        Ok(rows.iter().map(|row| row.get("record").ok()).collect())
    }

    /// Writes the node without a `uri`, keeping the stored one: for stubs and tombstones.
    pub async fn put_to_graph(&self) -> GraphResult<()> {
        exec_single_row(queries::put::create_user(self, None)?).await
    }

    /// A node created by key discovery and never read from a profile: it is
    /// named after its id. A profile name is at most 50 characters and an id
    /// 52, so no profile can take this shape.
    pub fn is_stub(&self) -> bool {
        self.name == self.id.as_ref()
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
            deleted: false,
            epoch: Epoch::V0,
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
            deleted: false,
            epoch: Epoch::V0,
        }
    }

    /// Cleared profile written when a user with relationships is deleted.
    /// Every field is wiped; `deleted` is the only signal. `name` is emptied
    /// rather than dropped because it is not optional.
    pub fn tombstone(user_id: &PubkyId) -> Self {
        UserDetails {
            name: String::new(),
            bio: None,
            id: user_id.clone(),
            links: None,
            status: None,
            image: None,
            indexed_at: Utc::now().timestamp_millis(),
            deleted: true,
            epoch: Epoch::V0,
        }
    }

    /// Writes a profile read from the homeserver, storing `uri` (its event path) on the node.
    pub async fn put_profile_to_graph(&self, uri: &str) -> GraphResult<()> {
        exec_single_row(queries::put::create_user(self, Some(uri))?).await
    }
}

/// Binds a user to their homeserver, recording the `HOSTED_BY` relationship and `resolved_at`.
pub async fn set_user_homeserver(user_id: &str, homeserver_id: &str) -> GraphResult<()> {
    exec_single_row(queries::put::set_user_homeserver(user_id, homeserver_id)).await
}

/// Toggles the stale flag on a user's existing homeserver mapping.
pub async fn set_user_homeserver_stale(user_id: &str, stale: bool) -> GraphResult<()> {
    exec_single_row(queries::put::set_user_homeserver_stale(user_id, stale)).await
}
/// True only when every id has a `User` node.
pub async fn users_exist(user_ids: &[&str]) -> GraphResult<bool> {
    let exist = fetch_key_from_graph(queries::get::users_exist(user_ids), "all_exist").await?;
    Ok(exist.unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use neo4rs::{BoltInteger, BoltList, BoltMap, BoltNode, BoltString, BoltType, Node};

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
        assert!(
            !details.deleted,
            "missing 'deleted' property should deserialize as false (pre-migration compatibility)"
        );
    }

    /// A present `deleted: true` deserializes as true — the other branch of the custom deserializer.
    #[test]
    fn deserialize_from_node_with_deleted_true() {
        let mut props = BoltMap::new();
        props.put(BoltString::from("name"), BoltType::from(""));
        props.put(
            BoltString::from("id"),
            BoltType::from("uo7jgkykft4885n8cruizwy6khw71mnu5pq3ay9i8pw1ymcn85ko"),
        );
        props.put(
            BoltString::from("indexed_at"),
            BoltType::from(1724134095000_i64),
        );
        props.put(BoltString::from("deleted"), BoltType::from(true));

        let node = Node::new(BoltNode::new(
            BoltInteger::new(1),
            BoltList::from(vec![BoltType::from("User")]),
            props,
        ));

        let details: UserDetails = node.to().expect("should deserialize with deleted: true");
        assert!(details.deleted);
    }

    /// A node without an epoch reads as `V0`; a stored `epoch` is respected.
    #[test]
    fn deserialize_epoch_from_node() {
        let node = |epoch: Option<&str>| {
            let mut props = BoltMap::new();
            props.put(BoltString::from("name"), BoltType::from("Dave"));
            props.put(
                BoltString::from("id"),
                BoltType::from("uo7jgkykft4885n8cruizwy6khw71mnu5pq3ay9i8pw1ymcn85ko"),
            );
            props.put(
                BoltString::from("indexed_at"),
                BoltType::from(1724134095000_i64),
            );
            if let Some(epoch) = epoch {
                props.put(BoltString::from("epoch"), BoltType::from(epoch));
            }
            Node::new(BoltNode::new(
                BoltInteger::new(1),
                BoltList::from(vec![BoltType::from("User")]),
                props,
            ))
        };

        for (stored, expected) in [(None, Epoch::V0), (Some("v1"), Epoch::V1)] {
            let details: UserDetails = node(stored).to().expect("should deserialize");
            assert_eq!(details.epoch, expected, "stored epoch {stored:?}");
        }
    }
}
