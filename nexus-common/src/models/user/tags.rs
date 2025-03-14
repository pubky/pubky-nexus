use super::UserDetails;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents a tag with its tag label, count, and author sources.
#[derive(Serialize, Deserialize, ToSchema, Default)]
pub struct ProfileTag {
    label: String,
    count: u32,
    by: Vec<UserDetails>,
}

/// Represents a collection of ProfileTag.
#[derive(Serialize, Deserialize, ToSchema, Default)]
pub struct UserTags {
    tags: Vec<ProfileTag>,
}

impl UserTags {
    /// TODO: Retrieves tags by user ID, currently returns an empty instance.
    pub async fn get_by_id(_user_id: &str) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        Ok(Some(Self::default()))
    }
}
