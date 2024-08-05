use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::{queries, RedisOps};
use async_trait::async_trait;
use neo4rs::Node;
use pkarr::PublicKey;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use utoipa::ToSchema;

/// Represents a user's single link with a title and URL.
#[derive(Serialize, Deserialize, ToSchema, Default, Clone)]
pub struct UserLink {
    title: String,
    url: String,
}

impl UserLink {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            url: String::new(),
        }
    }
}

/// Represents user data with name, bio, image, links, and status.
#[derive(Serialize, Deserialize, ToSchema, Default, Clone)]
pub struct UserDetails {
    name: String,
    bio: String,
    id: String,
    links: Vec<UserLink>,
    status: String,
    indexed_at: i64,
}

impl UserDetails {
    /// Retrieves details by user ID, first trying to get from Redis, then from Neo4j if not found.
    pub async fn get_by_id(
        user_id: &str,
    ) -> Result<Option<UserDetails>, Box<dyn std::error::Error + Send + Sync>> {
        // Delegate to UserDetailsCollection::get_by_ids for single item retrieval
        let details_collection = UserDetailsCollection::get_by_ids(&[user_id]).await?;
        Ok(details_collection.0.into_iter().flatten().next())
    }

    async fn from_node(node: &Node) -> Option<Self> {
        let id: String = node.get("id").unwrap_or_default();
        PublicKey::try_from(id.clone()).ok()?;

        Some(Self {
            id,
            name: node.get("name").unwrap_or_default(),
            bio: node.get("bio").unwrap_or_default(),
            status: node.get("status").unwrap_or_default(),
            links: node.get("links").unwrap_or_default(),
            indexed_at: node.get("indexed_at").unwrap_or_default(),
        })
    }
}

#[async_trait]
impl RedisOps for UserDetails {
    async fn prefix() -> String {
        String::from("User:Details")
    }
}

/// Represents a collection of UserDetails.
#[derive(Serialize, Deserialize)]
pub struct UserDetailsCollection(Vec<Option<UserDetails>>);

impl AsRef<[Option<UserDetails>]> for UserDetailsCollection {
    fn as_ref(&self) -> &[Option<UserDetails>] {
        &self.0
    }
}

// Implement Deref and DerefMut traits for UserDetailsCollection
impl Deref for UserDetailsCollection {
    type Target = Vec<Option<UserDetails>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for UserDetailsCollection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[async_trait]
impl RedisOps for UserDetailsCollection {
    async fn prefix() -> String {
        String::from("User:Details")
    }
}

impl UserDetailsCollection {
    /// Retrieves details for a list of user IDs, using Redis cache when available.
    pub async fn get_by_ids(
        user_ids: &[&str],
    ) -> Result<UserDetailsCollection, Box<dyn std::error::Error + Send + Sync>> {
        let key_parts_list: Vec<&[&str]> = user_ids.iter().map(std::slice::from_ref).collect();
        let mut user_details = UserDetailsCollection(
            UserDetails::try_from_index_multiple_json(&key_parts_list).await?,
        );

        let mut missing: Vec<(usize, &str)> = Vec::new();

        for (i, details) in user_details.iter().enumerate() {
            if details.is_none() {
                missing.push((i, user_ids[i]));
            }
        }

        if !missing.is_empty() {
            let missing_ids: Vec<&str> = missing.iter().map(|&(_, id)| id).collect();
            let fetched_details = Self::from_graph(&missing_ids).await?;

            for (i, (original_index, _)) in missing.iter().enumerate() {
                user_details[*original_index].clone_from(&fetched_details[i]);
            }
        }

        Ok(user_details)
    }

    /// Fetches user details from Neo4j and caches them in Redis.
    pub async fn from_graph(
        user_ids: &[&str],
    ) -> Result<UserDetailsCollection, Box<dyn std::error::Error + Send + Sync>> {
        let graph = get_neo4j_graph()?;
        let query = queries::get_users_details_by_ids(user_ids);

        let graph = graph.lock().await;
        let mut result = graph.execute(query).await?;
        let mut user_details = UserDetailsCollection(Vec::with_capacity(user_ids.len()));

        while let Some(row) = result.next().await? {
            let node: Option<Node> = row.get("u").ok();
            let detail = if let Some(n) = node {
                UserDetails::from_node(&n).await
            } else {
                None
            };
            user_details.push(detail);
        }

        // If new user details found from Graph, index them into Redis
        if !user_details.is_empty() {
            let mut existing_user_details = Vec::new();
            let mut existing_user_ids = Vec::new();

            for (detail, id) in user_details.iter().zip(user_ids.iter()) {
                if let Some(user_detail) = detail {
                    existing_user_details.push(Some(user_detail.clone()));
                    existing_user_ids.push(*id);
                }
            }

            let existing_user_details = UserDetailsCollection(existing_user_details);
            existing_user_details.to_index(&existing_user_ids).await?;
        }

        Ok(user_details)
    }

    /// Indexes user details in Redis.
    pub async fn to_index(
        &self,
        user_ids: &[&str],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Prepare key parts for valid user details
        let key_parts_list: Vec<Vec<&str>> = user_ids.iter().map(|id| vec![*id]).collect();
        let keys_refs: Vec<&[&str]> = key_parts_list.iter().map(|key| &key[..]).collect();

        self.put_multiple_json_indexes(&keys_refs).await
    }
}

#[cfg(test)]
mod tests {
    use crate::{setup, Config};

    use super::*;

    const USER_IDS: [&str; 8] = [
        "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro",
        "3iwsuz58pgrf7nw4kx8mg3fib1kqyi4oxqmuqxzsau1mpn5weipo",
        "3qgon1apkcmp63xbqpkrb3zzrja3nq9wou4u5bf7uu8rc9ehfo3y",
        "nope_it_does_not_exist", // Does not exist
        "4nacrqeuwh35kwrziy4m376uuyi7czazubgtyog4adm77ayqigxo",
        "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy",
        "4p1qa1ko7wuta4f1qm8io495cqsmefbgfp85wtnm9bj55gqbhjpo",
        "not_existing_user_id_either", // Does not exist
    ];

    #[tokio::test]
    async fn test_get_by_ids_from_redis() {
        let config = Config::from_env();
        setup(&config).await;

        let user_details = UserDetailsCollection::get_by_ids(&USER_IDS).await.unwrap();
        assert_eq!(user_details.len(), USER_IDS.len());

        for details in user_details[0..3].iter() {
            assert!(details.is_some());
        }
        for details in user_details[4..7].iter() {
            assert!(details.is_some());
        }
        assert!(user_details[3].is_none());
        assert!(user_details[7].is_none());

        assert_eq!(user_details[0].as_ref().unwrap().name, "Aldert");
        assert_eq!(user_details[5].as_ref().unwrap().name, "Flavio");

        for (i, details) in user_details.iter().enumerate() {
            if let Some(details) = details {
                assert_eq!(details.id, USER_IDS[i]);
            }
        }
    }
}
