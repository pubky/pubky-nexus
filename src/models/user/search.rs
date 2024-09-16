use std::error::Error;

use super::UserDetails;
use crate::RedisOps;
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
    ) -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
        // Convert the username to lowercase to ensure case-insensitive search
        let name = name.to_lowercase();

        let min = format!("[{}", name); // Inclusive range starting with "name"
        let max = format!("({}~", name); // Exclusive range ending just after "name"

        // Perform the lexicographical range search
        let elements =
            Self::try_from_index_sorted_set_lex(&USER_NAME_KEY_PARTS, &min, &max, skip, limit)
                .await?;

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

    /// Adds multiple `user_id`s to the Redis sorted set using the username as index.
    ///
    /// This method takes a list of `UserDetails` and adds them all to the sorted set at once.
    pub async fn add_many_to_username_sorted_set(
        details_list: &[&UserDetails],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Collect all the `username:user_id` pairs and their corresponding scores
        let mut items: Vec<(f64, String)> = Vec::with_capacity(details_list.len());

        for details in details_list {
            // Convert the username to lowercase before storing
            let username = details.name.to_lowercase();
            let user_id = &details.id;
            let score = 0.0;

            // The value in the sorted set will be `username:user_id`
            let member = format!("{}:{}", username, user_id.0);

            items.push((score, member));
        }

        // Perform a single Redis ZADD operation with all the items
        Self::put_index_sorted_set(
            &USER_NAME_KEY_PARTS,
            &items
                .iter()
                .map(|(score, member)| (*score, member.as_str()))
                .collect::<Vec<_>>(),
        )
        .await
    }
}
