use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::db::kv::index::json::JsonAction;
use crate::models::tag::user::USER_TAGS_KEY_PARTS;
use crate::types::DynError;
use crate::{queries, RedisOps};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::UserStream;

/// Represents total counts of relationships of a user.
#[derive(Serialize, Deserialize, ToSchema, Debug, Default)]
pub struct UserCounts {
    // The number of tags assigned to other entities (e.g. user, posts)
    pub tagged: u32,
    // User received tags counts
    pub tags: u32,
    // Distinct tags where the user was referenced
    pub unique_tags: u32,
    pub posts: u32,
    pub replies: u32,
    pub following: u32,
    pub followers: u32,
    pub friends: u32,
    pub bookmarks: u32,
}

impl RedisOps for UserCounts {}

impl UserCounts {
    /// Retrieves counts by user ID, first trying to get from Redis, then from Neo4j if not found.
    pub async fn get_by_id(user_id: &str) -> Result<Option<UserCounts>, DynError> {
        // TODO: uncomment the get_from_index approach when index counting is stable

        // match Self::get_from_index(user_id).await? {
        //     Some(counts) => Ok(Some(counts)),
        //     None => {
        //         let graph_response = Self::get_from_graph(user_id).await?;
        //         if let Some(user_counts) = graph_response {
        //             user_counts.put_to_index(user_id).await?;
        //             return Ok(Some(user_counts));
        //         }
        //         Ok(None)
        //     }
        // }
        Self::get_from_graph(user_id).await
    }

    /// Retrieves the counts from Neo4j.
    pub async fn get_from_graph(user_id: &str) -> Result<Option<UserCounts>, DynError> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query = queries::get::user_counts(user_id);

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        if let Some(row) = result.next().await? {
            let user_exists: bool = row.get("exists").unwrap_or(false);
            if user_exists {
                match row.get("counts") {
                    Ok(user_counts) => return Ok(Some(user_counts)),
                    // Like this we give a chance, in the next request to populate index
                    // If we populate the cache with default value, from that point we will have
                    // inconsistent state
                    Err(_e) => return Ok(None),
                }
            }
        }
        Ok(None)
    }

    pub async fn get_from_index(user_id: &str) -> Result<Option<UserCounts>, DynError> {
        if let Some(user_counts) = Self::try_from_index_json(&[user_id], None).await? {
            return Ok(Some(user_counts));
        }
        Ok(None)
    }

    pub async fn put_to_index(&self, user_id: &str) -> Result<(), DynError> {
        self.put_index_json(&[user_id], None, None).await?;
        UserStream::add_to_most_followed_sorted_set(user_id, self).await?;
        UserStream::add_to_pioneers_sorted_set(user_id, self).await?;
        Ok(())
    }

    pub async fn update_index_field(
        author_id: &str,
        field: &str,
        action: JsonAction,
    ) -> Result<(), DynError> {
        Self::modify_json_field(&[author_id], field, action).await?;
        Ok(())
    }

    pub async fn update(
        user_id: &str,
        field: &str,
        action: JsonAction,
        tag_label: Option<&str>,
    ) -> Result<(), DynError> {
        if let Some(label) = tag_label {
            let index_parts = [&USER_TAGS_KEY_PARTS[..], &[user_id]].concat();
            let score = Self::check_sorted_set_member(None, &index_parts, &[label]).await?;
            match (score, &action) {
                (Some(tag_value), JsonAction::Decrement(_)) if tag_value < 1 => (),
                (None, JsonAction::Increment(_)) => (),
                _ => return Ok(()),
            }
        }
        // Update user counts index
        Self::update_index_field(user_id, field, action).await?;
        // Just update pioneer and most followed indexes, when that fields are updated
        if field == "followers" || field == "tags" || field == "posts" {
            let exist_count = Self::get_by_id(user_id).await?;
            if let Some(user_counts) = exist_count {
                UserStream::add_to_pioneers_sorted_set(user_id, &user_counts).await?;
                // Increment followers
                if field == "followers" {
                    UserStream::add_to_most_followed_sorted_set(user_id, &user_counts).await?
                }
            }
        }
        Ok(())
    }

    pub async fn reindex(author_id: &str) -> Result<(), DynError> {
        match Self::get_from_graph(author_id).await? {
            Some(counts) => counts.put_to_index(author_id).await?,
            None => log::error!("{}: Could not found user counts in the graph", author_id),
        }
        Ok(())
    }

    pub async fn delete(user_id: &str) -> Result<(), DynError> {
        // Delete user_details on Redis
        Self::remove_from_index_multiple_json(&[&[user_id]]).await?;

        Ok(())
    }
}
