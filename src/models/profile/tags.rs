use super::ProfileDetails;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents a tag with its tag label, count, and author sources.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct ProfileTag {
    label: String,
    count: u32,
    by: Vec<ProfileDetails>,
}

impl Default for ProfileTag {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfileTag {
    pub fn new() -> Self {
        Self {
            label: String::new(),
            count: 0,
            by: vec![ProfileDetails::new()],
        }
    }
}

/// Represents a collection of ProfileTag.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct ProfileTags {
    tags: Vec<ProfileTag>,
}

impl Default for ProfileTags {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfileTags {
    pub fn new() -> Self {
        Self { tags: Vec::new() }
    }

    /// TODO: Retrieves tags by user ID, currently returns an empty instance.
    pub async fn get_by_id(_user_id: &str) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        Ok(Some(Self::new()))
    }
}
