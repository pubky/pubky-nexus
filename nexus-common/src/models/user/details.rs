use super::UserSearch;
use crate::db::kv::RedisResult;
use crate::db::{exec_single_row, queries, RedisOps};
use crate::models::traits::Collection;
use crate::types::DynError;
use async_trait::async_trait;
use chrono::Utc;
use neo4rs::Query;
use pubky_app_specs::{PubkyAppUser, PubkyAppUserLink, PubkyId};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json;
use utoipa::ToSchema;

#[async_trait]
impl RedisOps for UserDetails {}

#[async_trait]
impl Collection<&str> for UserDetails {
    fn collection_details_graph_query(id_list: &[&str]) -> Query {
        queries::get::get_users_details_by_ids(id_list)
    }

    fn put_graph_query(&self) -> Result<Query, DynError> {
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
#[derive(Serialize, Deserialize, ToSchema, Default, Clone, Debug)]
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
    pub async fn get_by_id(user_id: &str) -> Result<Option<Self>, DynError> {
        // Delegate to UserDetailsCollection::get_by_ids for single item retrieval
        let details_collection = Self::get_by_ids(&[user_id]).await?;
        Ok(details_collection.into_iter().flatten().next())
    }

    pub async fn from_homeserver(
        homeserver_user: PubkyAppUser,
        user_id: &PubkyId,
    ) -> Result<Self, DynError> {
        Ok(UserDetails {
            name: homeserver_user.name,
            bio: homeserver_user.bio,
            status: homeserver_user.status,
            links: homeserver_user.links,
            image: homeserver_user.image,
            id: user_id.clone(),
            indexed_at: Utc::now().timestamp_millis(),
        })
    }

    pub async fn delete(user_id: &str) -> Result<(), DynError> {
        // Delete user_details on Redis
        Self::remove_from_index_multiple_json(&[&[user_id]]).await?;
        // Delete user graph node;
        exec_single_row(queries::del::delete_user(user_id)).await?;

        Ok(())
    }
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
        props.put(BoltString::from("id"), BoltType::from("rz6oe4yda9em"));
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
}
