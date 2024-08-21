use super::UserSearch;
use crate::models::traits::Collection;
use crate::{queries, RedisOps};
use axum::async_trait;
use neo4rs::Query;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json;
use utoipa::ToSchema;

/// Represents a user's single link with a title and URL.
#[derive(Serialize, Deserialize, ToSchema, Default, Clone, Debug)]
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

#[async_trait]
impl RedisOps for UserDetails {
    async fn prefix() -> String {
        String::from("User:Details")
    }
}

#[async_trait]
impl Collection for UserDetails {
    fn graph_query(id_list: &[&str]) -> Query {
        queries::get_users_details_by_ids(id_list)
    }

    async fn add_to_sorted_sets(details: &[std::option::Option<Self>]) {
        // Filter out None and collect only the references to UserDetails
        let user_details_refs: Vec<&UserDetails> = details
            .iter()
            .filter_map(|detail| detail.as_ref()) // Filter out None and unwrap Some
            .collect();

        // Pass the references to the add_many_to_username_sorted_set function
        UserSearch::add_many_to_username_sorted_set(&user_details_refs)
            .await
            .unwrap();
    }
}

/// Represents user data with name, bio, image, links, and status.
#[derive(Serialize, Deserialize, ToSchema, Default, Clone, Debug)]
pub struct UserDetails {
    pub name: String,
    bio: String,
    pub id: String,
    #[serde(deserialize_with = "deserialize_user_links")]
    links: Vec<UserLink>,
    status: String,
    pub indexed_at: i64,
}

fn deserialize_user_links<'de, D>(deserializer: D) -> Result<Vec<UserLink>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = String::deserialize(deserializer)?;
    let urls: Vec<UserLink> = serde_json::from_str(&s).map_err(serde::de::Error::custom)?;
    Ok(urls)
}

impl UserDetails {
    /// Retrieves details by user ID, first trying to get from Redis, then from Neo4j if not found.
    pub async fn get_by_id(
        user_id: &str,
    ) -> Result<Option<UserDetails>, Box<dyn std::error::Error + Send + Sync>> {
        // Delegate to UserDetailsCollection::get_by_ids for single item retrieval
        let details_collection = Self::get_by_ids(&[user_id]).await?;
        Ok(details_collection.into_iter().flatten().next())
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

        let user_details = UserDetails::get_by_ids(&USER_IDS).await.unwrap();
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

        assert_eq!(user_details[5].as_ref().unwrap().links.len(), 4);
        assert_eq!(user_details[0].as_ref().unwrap().links.len(), 2);

        for (i, details) in user_details.iter().enumerate() {
            if let Some(details) = details {
                assert_eq!(details.id, USER_IDS[i]);
            }
        }
    }
}
