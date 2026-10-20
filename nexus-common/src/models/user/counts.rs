use crate::db::kv::{JsonAction, RedisResult};
use crate::db::{fetch_all_rows_from_graph, queries, RedisOps};
use crate::models::error::ModelResult;
use crate::models::tag::user::USER_TAGS_KEY_PARTS;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

use super::{is_stub, UserStream};

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
    // Defaulted so counts cached before this field existed still deserialize.
    #[serde(default)]
    pub collections: u32,
    pub following: u32,
    pub followers: u32,
    pub friends: u32,
    pub bookmarks: u32,
}

impl RedisOps for UserCounts {}

impl UserCounts {
    /// Counts are a graph COUNT: Redis is neither read nor refilled here.
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

    pub async fn get_from_index(user_id: &str) -> RedisResult<Option<UserCounts>> {
        Self::try_from_index_json(&[user_id], None).await
    }

    pub async fn put_to_index(&self, user_id: &str) -> RedisResult<()> {
        self.put_index_json(&[user_id], None, None).await?;
        // A stub's profile is never read: ranked, it would show as a blank card.
        if is_stub(user_id).await? {
            return Ok(());
        }
        UserStream::add_to_most_followed_sorted_set(user_id, self).await?;
        UserStream::add_to_influencers_sorted_set(user_id, self).await
    }

    pub async fn update_index_field(
        author_id: &str,
        field: &str,
        action: JsonAction,
    ) -> RedisResult<()> {
        Self::modify_json_field(&[author_id], field, action).await
    }

    /// Updates a user's counts index field and conditionally updates ranking sets
    /// based on follower, tag, or post counts.
    ///
    /// # Arguments
    ///
    /// * `user_id` - The unique identifier of the user whose index field is being updated.
    /// * `field` - The name of the user-related field to update (e.g., `"followers"`, `"tags"`, `"posts"`).
    /// * `action` - The action to perform on the field (increment or decrement).
    /// * `tag_label` - An optional tag label used to check membership in the user's tag-related sorted set. Important if we want to update the unique_tags field
    ///
    /// # Behavior
    ///
    /// - Conditional Update Based on `tag_label`
    /// - Update User Counts Index
    /// - Update Ranking Sets for Specific Fields
    pub async fn update(
        user_id: &str,
        field: &str,
        action: JsonAction,
        tag_label: Option<&str>,
    ) -> ModelResult<()> {
        // This condition applies only when updating `unique_tags`
        if let Some(label) = tag_label {
            let index_parts = [&USER_TAGS_KEY_PARTS[..], &[user_id]].concat();
            let score = Self::check_sorted_set_member(None, &index_parts, &[label]).await?;
            match (score, &action) {
                // If tag value is less than 1, `unique_tags` can be incremented or decremented
                (Some(tag_value), _) if tag_value < 1 => (),

                // Incrementing `unique_tags` is also allowed when the tag value doesn't exist yet in the sorted set
                (None, JsonAction::Increment(_)) => (),

                // Do not update the index
                _ => return Ok(()),
            }
        }
        // Update user counts index
        Self::update_index_field(user_id, field, action).await?;
        // Just update influencer and most followed indexes, when that fields are updated
        let ranked_field = field == "followers" || field == "tags" || field == "posts";
        if ranked_field && !is_stub(user_id).await? {
            // Index only, never a read-through: a caller may still be mid-change in the graph
            // (an unfollow deletes its edge after this), and a graph read here would cache the
            // pre-change counts. With no counts key there is nothing to rank yet; reads
            // never refill it, only `UserCounts::reindex` rebuilds the key and both ranks.
            let exist_count = Self::get_from_index(user_id).await?;
            if let Some(user_counts) = exist_count {
                UserStream::add_to_influencers_sorted_set(user_id, &user_counts).await?;
                // Increment followers
                if field == "followers" {
                    UserStream::add_to_most_followed_sorted_set(user_id, &user_counts).await?
                }
            }
        }
        Ok(())
    }

    pub async fn reindex(author_id: &str) -> ModelResult<()> {
        match Self::get_by_id(author_id).await? {
            Some(counts) => counts.put_to_index(author_id).await?,
            None => tracing::error!("{}: Could not found user counts in the graph", author_id),
        }
        Ok(())
    }

    pub async fn delete(user_id: &str) -> RedisResult<()> {
        // Delete user_details on Redis
        Self::remove_from_index_multiple_json(&[&[user_id]]).await
    }

    /// Increments a field in a user's counts by 1.
    pub async fn increment(user_id: &str, field: &str, tag_label: Option<&str>) -> ModelResult<()> {
        Self::update(user_id, field, JsonAction::Increment(1), tag_label).await
    }

    /// Decrements a field in a user's counts by 1.
    pub async fn decrement(user_id: &str, field: &str, tag_label: Option<&str>) -> ModelResult<()> {
        Self::update(user_id, field, JsonAction::Decrement(1), tag_label).await
    }
}
