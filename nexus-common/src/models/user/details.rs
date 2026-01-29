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
    // Deserialize into serde_json::Value first
    let value = serde_json::Value::deserialize(deserializer)?;

    // Handle both cases
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

// #[cfg(test)]
// mod tests {

//     use super::*;
//     use crate::_service::NexusApi;

//     const USER_IDS: [&str; 8] = [
//         "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro",
//         "3iwsuz58pgrf7nw4kx8mg3fib1kqyi4oxqmuqxzsau1mpn5weipo",
//         "3qgon1apkcmp63xbqpkrb3zzrja3nq9wou4u5bf7uu8rc9ehfo3y",
//         "nope_it_does_not_exist", // Does not exist
//         "4nacrqeuwh35kwrziy4m376uuyi7czazubgtyog4adm77ayqigxo",
//         "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy",
//         "4p1qa1ko7wuta4f1qm8io495cqsmefbgfp85wtnm9bj55gqbhjpo",
//         "not_existing_user_id_either", // Does not exist
//     ];

//     #[tokio_shared_rt::test(shared)]
//     async fn test_get_by_ids_from_redis() {
//         NexusApi::builder().init_stack().await;

//         let user_details = UserDetails::get_by_ids(&USER_IDS).await.unwrap();
//         assert_eq!(user_details.len(), USER_IDS.len());

//         for details in user_details[0..3].iter() {
//             assert!(details.is_some());
//         }
//         for details in user_details[4..7].iter() {
//             assert!(details.is_some());
//         }
//         assert!(user_details[3].is_none());
//         assert!(user_details[7].is_none());

//         assert_eq!(user_details[0].as_ref().unwrap().name, "Aldert");
//         assert_eq!(user_details[5].as_ref().unwrap().name, "Flavio");

//         assert_eq!(
//             user_details[5]
//                 .as_ref()
//                 .unwrap()
//                 .links
//                 .as_ref()
//                 .unwrap()
//                 .len(),
//             4
//         );
//         assert_eq!(
//             user_details[0]
//                 .as_ref()
//                 .unwrap()
//                 .links
//                 .as_ref()
//                 .unwrap()
//                 .len(),
//             2
//         );

//         for (i, details) in user_details.iter().enumerate() {
//             if let Some(details) = details {
//                 assert_eq!(details.id.as_ref(), USER_IDS[i]);
//             }
//         }
//     }
// }
