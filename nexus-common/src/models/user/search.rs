use super::UserDetails;
use crate::db::RedisOps;
use crate::{models::traits::Collection, types::DynError};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const USER_NAME_KEY_PARTS: [&str; 2] = ["Users", "Name"];

#[derive(Serialize, Deserialize, ToSchema, Default)]
pub struct UserSearch(pub Vec<String>);

impl RedisOps for UserSearch {}

impl UserSearch {
    pub async fn get_by_name(
        name: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Self>, DynError> {
        // Perform the lexicographical range search
        let elements = Self::get_from_index(name, skip, limit).await?;

        // If elements exist, process them to extract user_ids
        if let Some(elements) = elements {
            let user_ids: Vec<String> = elements
                .into_iter()
                .filter_map(|element| {
                    // Split by `:` and take the last part (user_id)
                    element.split(':').next_back().map(|p| p.to_string())
                })
                .collect();

            return Ok(Some(UserSearch(user_ids)));
        }

        Ok(None)
    }

    pub async fn get_from_index(
        name: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Vec<String>>, DynError> {
        // Convert the username to lowercase to ensure case-insensitive search
        let name = name.to_lowercase();

        let min = format!("[{name}"); // Inclusive range starting with "name"
        let max = format!("({name}~"); // Exclusive range ending just after "name"

        // Perform the lexicographical range search
        Self::try_from_index_sorted_set_lex(&USER_NAME_KEY_PARTS, &min, &max, skip, limit).await
    }

    /// Adds multiple `user_id`s to the Redis sorted set using the username as index.
    ///
    /// This method takes a list of `UserDetails` and adds them all to the sorted set at once.
    pub async fn put_to_index(details_list: &[&UserDetails]) -> Result<(), DynError> {
        // ensure existing records are deleted
        Self::delete_existing_records(
            details_list
                .iter()
                .map(|details| details.id.as_str())
                .collect::<Vec<&str>>()
                .as_slice(),
        )
        .await?;

        // Collect all the `username:user_id` pairs and their corresponding scores
        let mut items: Vec<(f64, String)> = Vec::with_capacity(details_list.len());

        for details in details_list {
            // Convert the username to lowercase before storing
            if details.name == "[DELETED]" {
                break;
            }
            let username = details.name.to_lowercase();
            let user_id = &details.id;
            let score = 0.0;

            // The value in the sorted set will be `username:user_id`
            let member = format!("{username}:{user_id}");

            items.push((score, member));
        }

        // Perform a single Redis ZADD operation with all the items
        Self::put_index_sorted_set(
            &USER_NAME_KEY_PARTS,
            &items
                .iter()
                .map(|(score, member)| (*score, member.as_str()))
                .collect::<Vec<_>>(),
            None,
            None,
        )
        .await
    }

    async fn delete_existing_records(user_ids: &[&str]) -> Result<(), DynError> {
        if user_ids.is_empty() {
            return Ok(());
        }
        let mut records_to_delete: Vec<String> = Vec::with_capacity(user_ids.len());
        let keys = user_ids
            .iter()
            .map(|&id| vec![id])
            .collect::<Vec<Vec<&str>>>();
        let users = UserDetails::get_from_index(keys.iter().map(|item| item.as_slice()).collect())
            .await?
            .into_iter()
            .flatten()
            .collect::<Vec<UserDetails>>();
        for user_id in user_ids {
            let existing_username = users
                .iter()
                .find(|user| user.id.to_string() == *user_id)
                .map(|user| user.name.to_lowercase());
            if let Some(existing_record) = existing_username {
                let search_key = format!("{existing_record}:{user_id}");
                records_to_delete.push(search_key);
            }
        }

        Self::remove_from_index_sorted_set(
            None,
            &USER_NAME_KEY_PARTS,
            records_to_delete
                .iter()
                .map(|item| item.as_str())
                .collect::<Vec<&str>>()
                .as_slice(),
        )
        .await?;
        Ok(())
    }
}

// #[cfg(test)]
// mod tests {
//     use crate::{
//         models::{
//             traits::Collection,
//             user::{UserDetails, UserSearch},
//         },
//         types::DynError,
//         RedisOps,
//         _service::NexusApi,
//     };
//     use chrono::Utc;
//     use pubky_app_specs::PubkyId;

//     #[tokio_shared_rt::test(shared)]
//     async fn test_put_to_index_no_duplicates() -> Result<(), DynError> {
//         NexusApi::builder().init_stack().await;
//         // Test that the `put_to_index` method does not add duplicate records to the index
//         // when called with the same `UserDetails` multiple times.

//         // Create a `UserDetails` object
//         let user_id = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
//         let user_name = "Test User Duplicate";
//         let user_details = UserDetails {
//             id: PubkyId::try_from(user_id).expect("valid pubky id"),
//             name: user_name.to_string(),
//             bio: None,
//             status: None,
//             links: None,
//             image: None,
//             indexed_at: Utc::now().timestamp_millis(),
//         };

//         user_details.put_to_graph().await?;
//         user_details
//             .put_index_json(vec![user_id].as_slice(), None, None)
//             .await?;

//         // Call `put_to_index` with the same `UserDetails` object
//         UserSearch::put_to_index(&[&user_details]).await?;

//         // Check that the index contains only one record for the user
//         let search_result = UserSearch::get_by_name(&user_name, None, None).await?;
//         assert_eq!(search_result.unwrap().0, vec![user_id.to_string()]);

//         let new_user_name = "Some Other User Name";
//         let new_user_details = UserDetails {
//             id: PubkyId::try_from(user_id).expect("valid pubky id"),
//             name: new_user_name.to_string(),
//             bio: None,
//             status: None,
//             links: None,
//             image: None,
//             indexed_at: Utc::now().timestamp_millis(),
//         };

//         // Call `put_to_index` with new user details
//         UserSearch::put_to_index(&[&new_user_details]).await?;

//         // Check the previous record is deleted
//         // Check that the index contains only one record for the user
//         let search_result = UserSearch::get_by_name(&user_name, None, None).await?;
//         assert_eq!(search_result.is_none(), true);

//         Ok(())
//     }
//}
