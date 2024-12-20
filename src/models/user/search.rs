use super::UserDetails;
use crate::types::DynError;
use crate::RedisOps;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const USER_NAME_KEY_PARTS: [&str; 2] = ["Users", "Name"];
pub const USER_NAME_HASHMAP_KEY_PARTS: [&str; 3] = ["Hashmap", "Users", "Name"];

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
                    // Split by `:` and take the second part (user_id)
                    element
                        .split_once(':')
                        .map(|(_, user_id)| user_id.to_string())
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

        let min = format!("[{}", name); // Inclusive range starting with "name"
        let max = format!("({}~", name); // Exclusive range ending just after "name"

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
                .map(|details| details.id.0.as_str())
                .collect::<Vec<&str>>()
                .as_slice(),
        )
        .await?;

        // Collect all the `username:user_id` pairs and their corresponding scores
        let mut items: Vec<(f64, String)> = Vec::with_capacity(details_list.len());
        let mut hashmap_items: Vec<(String, String)> = Vec::with_capacity(details_list.len());

        for details in details_list {
            // Convert the username to lowercase before storing
            if details.name == "[DELETED]" {
                break;
            }
            let username = details.name.to_lowercase();
            let user_id = &details.id;
            let score = 0.0;

            // The value in the sorted set will be `username:user_id`
            let member = format!("{}:{}", username, user_id.0);

            hashmap_items.push((user_id.0.clone(), username.clone()));
            items.push((score, member));
        }

        // put the items in hashmap for unique index
        Self::put_index_hashmap(
            &USER_NAME_HASHMAP_KEY_PARTS,
            &hashmap_items
                .iter()
                .map(|(user_id, username)| (user_id.as_str(), username.as_str()))
                .collect::<Vec<(&str, &str)>>(),
        )
        .await?;
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
        let mut records_to_delete: Vec<String> = Vec::with_capacity(user_ids.len());
        for user_id in user_ids {
            let existing_record =
                Self::try_from_index_hashmap(&USER_NAME_HASHMAP_KEY_PARTS, user_id).await?;
            if let Some(existing_record) = existing_record {
                let search_key = format!("{}:{}", existing_record, user_id);
                records_to_delete.push(search_key);
            }
        }

        Self::remove_from_index_sorted_set(
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

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use crate::{
        models::user::{UserDetails, UserSearch},
        setup,
        types::{DynError, PubkyId},
        Config,
    };

    #[tokio_shared_rt::test(shared)]
    async fn test_put_to_index_no_duplicates() -> Result<(), DynError> {
        let config = Config::from_env();
        setup(&config).await;
        // Test that the `put_to_index` method does not add duplicate records to the index
        // when called with the same `UserDetails` multiple times.

        // Create a `UserDetails` object
        let user_id = "user_id";
        let user_name = "Test User Duplicate";
        let user_details = UserDetails {
            id: PubkyId(user_id.to_string()),
            name: user_name.to_string(),
            bio: None,
            status: None,
            links: None,
            image: None,
            indexed_at: Utc::now().timestamp_millis(),
        };

        // Call `put_to_index` with the same `UserDetails` object multiple times
        UserSearch::put_to_index(&[&user_details]).await?;

        // Check that the index contains only one record for the user
        let search_result = UserSearch::get_by_name(&user_name, None, None).await?;
        assert_eq!(search_result.unwrap().0, vec![user_id.to_string()]);

        let new_user_name = "Test User Duplicate 2";
        let new_user_details = UserDetails {
            id: PubkyId(user_id.to_string()),
            name: new_user_name.to_string(),
            bio: None,
            status: None,
            links: None,
            image: None,
            indexed_at: Utc::now().timestamp_millis(),
        };

        // Call `put_to_index` with new user details
        UserSearch::put_to_index(&[&new_user_details]).await?;

        // Check the previous record is deleted
        // Check that the index contains only one record for the user
        let search_result = UserSearch::get_by_name(&user_name, None, None).await?;
        let empty_result: Vec<String> = vec![];
        assert_eq!(search_result.unwrap().0, empty_result);

        Ok(())
    }
}
