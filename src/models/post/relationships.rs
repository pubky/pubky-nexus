use crate::models::Prefix;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::models::profile::ProfileDetails;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct PostRelationships {
    reply_of: String,
    repost_of: String,
    mentions: Vec<ProfileDetails>,
}

impl Default for PostRelationships {
    fn default() -> Self {
        Self::new()
    }
}

impl PostRelationships {
    pub fn new() -> Self {
        Self {
            reply_of: String::new(),
            repost_of: String::new(),
            mentions: vec![ProfileDetails::new()],
        }
    }
}
