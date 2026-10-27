use crate::db::{fetch_all_rows_from_graph, queries};
use crate::models::error::ModelResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

/// Represents total counts of relationships of a user.
#[derive(Serialize, Deserialize, ToSchema, Debug, Default, Clone)]
pub struct UserCounts {
    // The number of tags assigned to other entities by the user (e.g. user, posts)
    pub tagged: u32,
    // User received tags counts
    pub tags: u32,
    // Distinct tags where the user was referenced
    pub unique_tags: u32,
    pub posts: u32,
    pub replies: u32,
    pub collections: u32,
    pub following: u32,
    pub followers: u32,
    pub friends: u32,
    pub bookmarks: u32,
}

impl UserCounts {
    /// Counts are a graph COUNT per field; nothing caches them.
    pub async fn get_by_id(user_id: &str) -> ModelResult<Option<UserCounts>> {
        Ok(Self::get_by_ids(&[user_id]).await?.pop().flatten())
    }

    /// One graph round-trip for a page of users; one slot per id, `None` for
    /// ids with no `User` node.
    pub async fn get_by_ids<T: AsRef<str>>(user_ids: &[T]) -> ModelResult<Vec<Option<UserCounts>>> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<&str> = user_ids.iter().map(AsRef::as_ref).collect();
        let rows = fetch_all_rows_from_graph(queries::get::user_counts_by_ids(&ids)).await?;

        let mut by_id: HashMap<String, UserCounts> = HashMap::with_capacity(rows.len());
        for row in rows {
            if row.get::<bool>("exists")? {
                by_id.insert(row.get("id")?, row.get("counts")?);
            }
        }
        Ok(ids.iter().map(|id| by_id.get(*id).cloned()).collect())
    }
}
