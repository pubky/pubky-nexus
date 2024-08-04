use crate::models::user::UserDetails;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents a tag with its tag label, count, and author sources.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct PostTag {
    label: String,
    count: u32,
    by: Vec<UserDetails>,
}

impl Default for PostTag {
    fn default() -> Self {
        Self::new()
    }
}

impl PostTag {
    pub fn new() -> Self {
        Self {
            label: String::new(),
            count: 0,
            by: vec![UserDetails::default()],
        }
    }
}

/// Represents a collection of PostTag.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct PostTags {
    tags: Vec<PostTag>,
}

impl Default for PostTags {
    fn default() -> Self {
        Self::new()
    }
}

impl PostTags {
    pub fn new() -> Self {
        Self { tags: Vec::new() }
    }

    /// TODO: Retrieves tags by user ID, currently returns an empty instance.
    pub async fn get_by_id(_user_id: &str) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        Ok(Some(Self::new()))
    }
}
