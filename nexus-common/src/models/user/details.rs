use super::Epoch;
use crate::db::{
    exec_single_row, fetch_all_rows_from_graph, fetch_key_from_graph, queries, GraphResult,
    RedisOps,
};
use crate::models::error::ModelResult;
use chrono::Utc;
use neo4rs::Row;
use pubky_social_specs::PubkyId;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json;
use std::collections::HashMap;
use tracing::warn;
use utoipa::ToSchema;

/// One link on a profile, in Nexus vocabulary: every epoch's translator maps its spec's links
/// into it, after the spec reader has validated and sanitised them.
#[derive(Serialize, Deserialize, ToSchema, Clone, Debug, PartialEq, Eq)]
pub struct UserLink {
    pub title: String,
    pub url: String,
}

/// Represents user data with name, bio, image, links, and status.
// Read from the graph, never cached: see `get_by_ids`.
#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct UserDetails {
    pub name: String,
    pub bio: Option<String>,
    pub id: PubkyId,
    /// The profile's links, in order; empty when it has none.
    #[serde(deserialize_with = "deserialize_user_links", default)]
    #[schema(required = true)]
    pub links: Vec<UserLink>,
    pub status: Option<String>,
    pub image: Option<String>,
    pub indexed_at: i64,
    #[serde(deserialize_with = "deserialize_user_deleted", default)]
    pub deleted: bool,
    #[serde(deserialize_with = "deserialize_user_epoch", default)]
    pub epoch: Epoch,
}

// Only for the migrations that read or clean up the documents this type was
// once cached in: the shipped `UserDeletedFlag1780617600` and
// `UserEpochBackfill1791812997`, which are never edited, and
// `OverwrittenProfileRestore1795036051`, which restores profiles from them.
// Nothing else reads or writes those documents.
impl RedisOps for UserDetails {}

/// Reads every stored form of the links, and every form without any as an empty list: the
/// graph's JSON string (`"null"` on profiles indexed before links were always a list), a
/// JSON array or `null`, or no property at all.
fn deserialize_user_links<'de, D>(deserializer: D) -> Result<Vec<UserLink>, D::Error>
where
    D: Deserializer<'de>,
{
    // Deserialize as Option to handle missing properties in neo4rs.
    // Neo4j drops null properties from nodes, so when a node lacks the links
    // property, neo4rs provides a fallback deserializer that only handles
    // deserialize_option (returning None), not deserialize_any.
    let value = match Option::<serde_json::Value>::deserialize(deserializer)? {
        Some(v) => v,
        None => return Ok(Vec::new()),
    };

    match value {
        serde_json::Value::String(s) => {
            // If it's a string, parse the string as JSON
            let links: Option<Vec<UserLink>> =
                serde_json::from_str(&s).map_err(serde::de::Error::custom)?;
            Ok(links.unwrap_or_default())
        }
        serde_json::Value::Array(arr) => {
            // If it's already an array, deserialize it directly
            serde_json::from_value(serde_json::Value::Array(arr)).map_err(serde::de::Error::custom)
        }
        serde_json::Value::Null => Ok(Vec::new()),
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
    /// Reads the user's details from the graph: `None` when there is no such
    /// node, or when the node does not deserialize (see [`Self::from_row`]).
    pub async fn get_by_id(user_id: &str) -> ModelResult<Option<Self>> {
        Ok(Self::get_by_ids(&[user_id]).await?.pop().flatten())
    }

    /// Reads each user's details from the graph: one slot per id, in order,
    /// `None` where there is no such node or it does not deserialize (see
    /// [`Self::from_row`]); callers that filter rankings evict on `None`.
    pub async fn get_by_ids(user_ids: &[impl AsRef<str>]) -> ModelResult<Vec<Option<Self>>> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<&str> = user_ids.iter().map(AsRef::as_ref).collect();
        let rows = fetch_all_rows_from_graph(queries::get::get_users_details_by_ids(&ids)).await?;
        let found = rows
            .iter()
            .map(Self::from_row)
            .collect::<GraphResult<Vec<_>>>()?;
        Ok(match_to_ids(&ids, found))
    }

    /// The `id` and the `record` node of a row: the node is `None` when
    /// missing, and when it does not deserialize, which is logged with its id
    /// rather than failing the whole read.
    pub fn from_row(row: &Row) -> GraphResult<(String, Option<Self>)> {
        let id: String = row.get("id")?;
        let record = match row.get::<Option<Self>>("record") {
            Ok(record) => record,
            Err(error) => {
                warn!(user_id = %id, %error, "Skipping a user node that does not deserialize");
                None
            }
        };
        Ok((id, record))
    }

    /// Splits `items` by the details read for them, in order: the ones whose
    /// user is live, and the ones to evict, whose user is tombstoned or gone.
    pub fn split_live<T>(items: Vec<T>, details: Vec<Option<Self>>) -> (Vec<T>, Vec<T>) {
        let mut live = Vec::with_capacity(items.len());
        let mut evicted = Vec::new();
        for (item, details) in items.into_iter().zip(details) {
            match details {
                Some(details) if !details.deleted => live.push(item),
                _ => evicted.push(item),
            }
        }
        (live, evicted)
    }

    /// Writes the node without a `uri`, keeping the stored one, and overwrites every other
    /// profile field. Key discovery must not use it: see [`Self::create_stub_in_graph`].
    pub async fn put_to_graph(&self) -> GraphResult<()> {
        exec_single_row(queries::put::create_user(self, None)?).await
    }

    /// Creates `user_id`'s node as the stub [`Self::from_pubky`] gives, unless the user has a
    /// node: a profile indexed since the caller last looked is kept as it is.
    pub async fn create_stub_in_graph(user_id: &PubkyId) -> GraphResult<()> {
        let stub = Self::from_pubky(user_id.clone());
        exec_single_row(queries::put::create_user_stub(&stub)).await
    }

    /// Creates a minimal `UserDetails` with only the public key: the stub of a user whose
    /// profile Nexus has not indexed, named by its id. Every profile field is empty: bio, status and
    /// image are `None`, the links an empty list. The epoch write's `ON CREATE` ([`Epoch::set_v1_in_graph`])
    /// mirrors it, so a stub looks the same whichever path created it.
    pub fn from_pubky(user_id: PubkyId) -> Self {
        UserDetails {
            name: user_id.to_string(),
            id: user_id.clone(),
            indexed_at: Utc::now().timestamp_millis(),
            bio: None,
            links: Vec::new(),
            status: None,
            image: None,
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
            links: Vec::new(),
            status: None,
            image: None,
            indexed_at: Utc::now().timestamp_millis(),
            deleted: true,
            epoch: Epoch::V0,
        }
    }

    /// Writes a profile read from the homeserver, storing `uri` (its event path) on the node.
    ///
    /// The write keeps the epoch stored on the node, and sets it on `self`.
    pub async fn put_profile_to_graph(&mut self, uri: &str) -> GraphResult<()> {
        let query = queries::put::create_user(self, Some(uri))?;
        self.epoch = Epoch::fetch_from_graph(query).await?;
        Ok(())
    }

    /// Writes a [`UserDetails::tombstone`], keeping the stored `uri`. The write keeps the epoch
    /// stored on the node, and sets it on `self`.
    pub async fn put_tombstone_to_graph(&mut self) -> GraphResult<()> {
        let query = queries::put::create_user(self, None)?;
        self.epoch = Epoch::fetch_from_graph(query).await?;
        Ok(())
    }
}

/// Gives each id the details of the row that carries it, as the query keeps no
/// order: `None` where no row brought details, and the same details in every
/// slot of a repeated id.
fn match_to_ids(
    ids: &[&str],
    found: Vec<(String, Option<UserDetails>)>,
) -> Vec<Option<UserDetails>> {
    let by_id: HashMap<String, UserDetails> = found
        .into_iter()
        .filter_map(|(id, details)| Some((id, details?)))
        .collect();
    ids.iter().map(|id| by_id.get(*id).cloned()).collect()
}

/// Binds a user to a homeserver without a PKDNS record, as tests do, recording
/// the `HOSTED_BY` relationship and `resolved_at`; the resolver and the ingestor
/// bind from the record they resolved, see [`set_user_homeserver_from_record`].
pub async fn set_user_homeserver(user_id: &str, homeserver_id: &str) -> GraphResult<()> {
    exec_single_row(queries::put::set_user_homeserver(
        user_id,
        homeserver_id,
        None,
    ))
    .await
}

/// Binds a user to the homeserver their PKDNS record names, keeping the record
/// on the `HOSTED_BY` relationship.
pub async fn set_user_homeserver_from_record(
    user_id: &str,
    homeserver_id: &str,
    record: &super::StoredRecord,
) -> GraphResult<()> {
    exec_single_row(queries::put::set_user_homeserver(
        user_id,
        homeserver_id,
        Some(record),
    ))
    .await
}

/// Toggles the stale flag on a user's existing homeserver mapping, keeping the
/// PKDNS record the flag was applied from on the relationship when there is one.
pub async fn set_user_homeserver_stale(
    user_id: &str,
    stale: bool,
    record: Option<&super::StoredRecord>,
) -> GraphResult<()> {
    exec_single_row(queries::put::set_user_homeserver_stale(
        user_id, stale, record,
    ))
    .await
}
/// True only when every id has a `User` node.
pub async fn users_exist(user_ids: &[&str]) -> GraphResult<bool> {
    let exist = fetch_key_from_graph(queries::get::users_exist(user_ids), "all_exist").await?;
    Ok(exist.unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::random_pubky_id;
    use neo4rs::{BoltInteger, BoltList, BoltMap, BoltNode, BoltString, BoltType, Node};
    use tracing_test::traced_test;

    fn ids_of(slots: &[Option<UserDetails>]) -> Vec<Option<String>> {
        slots
            .iter()
            .map(|slot| slot.as_ref().map(|details| details.id.to_string()))
            .collect()
    }

    /// Rows in any order: each id gets the details whose row carries it.
    #[test]
    fn match_to_ids_follows_the_ids_not_the_rows() {
        let first = UserDetails::from_pubky(random_pubky_id());
        let second = UserDetails::from_pubky(random_pubky_id());
        let ids = [first.id.as_ref(), second.id.as_ref()];
        let rows = vec![
            (second.id.to_string(), Some(second.clone())),
            (first.id.to_string(), Some(first.clone())),
        ];
        let slots = match_to_ids(&ids, rows);
        assert_eq!(
            ids_of(&slots),
            vec![Some(first.id.to_string()), Some(second.id.to_string())]
        );
    }

    /// An id whose row brought no details gets `None`; the others are kept.
    #[test]
    fn match_to_ids_leaves_a_missing_id_empty() {
        let missing = random_pubky_id().to_string();
        let found = UserDetails::from_pubky(random_pubky_id());
        let ids = [missing.as_str(), found.id.as_ref()];
        let rows = vec![
            (found.id.to_string(), Some(found.clone())),
            (missing.clone(), None),
        ];
        let slots = match_to_ids(&ids, rows);
        assert_eq!(ids_of(&slots), vec![None, Some(found.id.to_string())]);
    }

    /// An id asked for twice gets the details in both of its slots.
    #[test]
    fn match_to_ids_fills_each_slot_of_a_repeated_id() {
        let user = UserDetails::from_pubky(random_pubky_id());
        let ids = [user.id.as_ref(), user.id.as_ref()];
        let rows = vec![(user.id.to_string(), Some(user.clone()))];
        let slots = match_to_ids(&ids, rows);
        assert_eq!(
            ids_of(&slots),
            vec![Some(user.id.to_string()), Some(user.id.to_string())]
        );
    }

    /// Deserializing a UserDetails from a BoltNode without the links property
    /// should succeed with no links. Neo4j drops null properties from nodes,
    /// so a node written without links has this shape.
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
        assert!(details.links.is_empty());
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

    /// A live user is kept; a tombstone and a missing node are evicted.
    #[test]
    fn split_live_keeps_live_users_only() {
        let id = PubkyId::try_from("uo7jgkykft4885n8cruizwy6khw71mnu5pq3ay9i8pw1ymcn85ko").unwrap();
        let details = vec![
            Some(UserDetails::from_pubky(id.clone())),
            Some(UserDetails::tombstone(&id)),
            None,
        ];
        let (live, evicted) = UserDetails::split_live(vec!["live", "tombstone", "gone"], details);
        assert_eq!(live, vec!["live"]);
        assert_eq!(evicted, vec!["tombstone", "gone"]);
    }

    /// A null record is a missing node and logs nothing, a valid node reads,
    /// and a node that does not deserialize is skipped with its id logged.
    #[traced_test]
    #[test]
    fn from_row_tells_a_missing_node_from_a_malformed_one() {
        let id = "uo7jgkykft4885n8cruizwy6khw71mnu5pq3ay9i8pw1ymcn85ko";
        let node = |indexed_at: Option<i64>| {
            let mut props = BoltMap::new();
            props.put(BoltString::from("name"), BoltType::from("Dave"));
            props.put(BoltString::from("id"), BoltType::from(id));
            if let Some(indexed_at) = indexed_at {
                props.put(BoltString::from("indexed_at"), BoltType::from(indexed_at));
            }
            BoltType::Node(BoltNode::new(
                BoltInteger::new(1),
                BoltList::from(vec![BoltType::from("User")]),
                props,
            ))
        };
        let row = |record: BoltType| {
            let fields = BoltList::from(vec![BoltType::from("id"), BoltType::from("record")]);
            let data = BoltList::from(vec![BoltType::from(id), record]);
            Row::new(fields, data)
        };

        let record = |record: BoltType| {
            let (row_id, details) = UserDetails::from_row(&row(record)).expect("the row has an id");
            assert_eq!(row_id, id);
            details
        };

        assert!(record(BoltType::Null(neo4rs::BoltNull)).is_none());
        assert!(!logs_contain("does not deserialize"));

        let details = record(node(Some(1724134095000))).expect("a valid node");
        assert_eq!(details.name, "Dave");

        assert!(record(node(None)).is_none());
        assert!(logs_contain(
            "Skipping a user node that does not deserialize"
        ));
        assert!(logs_contain(id));
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

    /// Every stored form of the links reads as a list; the ones without links as an empty one.
    #[test]
    fn deserialize_links_in_every_stored_form() {
        let link = UserLink {
            title: "site".into(),
            url: "https://example.com".into(),
        };
        let with = |links: Option<serde_json::Value>| {
            let mut json = serde_json::json!({
                "name": "Dave",
                "bio": null,
                "id": "uo7jgkykft4885n8cruizwy6khw71mnu5pq3ay9i8pw1ymcn85ko",
                "status": null,
                "image": null,
                "indexed_at": 1724134095000_i64,
            });
            if let Some(links) = links {
                json["links"] = links;
            }
            serde_json::from_value::<UserDetails>(json).expect("should deserialize")
        };
        let listed = serde_json::json!([{ "title": "site", "url": "https://example.com" }]);

        for (stored, expected) in [
            (None, vec![]),
            (Some(serde_json::Value::Null), vec![]),
            (Some(serde_json::json!("null")), vec![]),
            (Some(serde_json::json!("[]")), vec![]),
            (Some(serde_json::json!([])), vec![]),
            (
                Some(serde_json::json!(listed.to_string())),
                vec![link.clone()],
            ),
            (Some(listed.clone()), vec![link.clone()]),
        ] {
            assert_eq!(with(stored.clone()).links, expected, "stored {stored:?}");
        }
    }

    /// The stub and the tombstone store an empty list, never `null`.
    #[test]
    fn stub_and_tombstone_store_an_empty_list() {
        let id = PubkyId::try_from("uo7jgkykft4885n8cruizwy6khw71mnu5pq3ay9i8pw1ymcn85ko").unwrap();
        for details in [
            UserDetails::from_pubky(id.clone()),
            UserDetails::tombstone(&id),
        ] {
            assert_eq!(
                serde_json::to_value(&details).unwrap()["links"],
                serde_json::json!([])
            );
        }
    }
}
