use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::Timeframe;
use crate::models::tag::{TaggedType, Taggers};

pub struct HotTagsInput {
    pub timeframe: Timeframe,
    pub skip: usize,
    pub limit: usize,
    pub taggers_limit: usize,
    pub tagged_type: Option<TaggedType>,
}

impl HotTagsInput {
    pub fn new(
        timeframe: Timeframe,
        limit: usize,
        skip: usize,
        taggers_limit: usize,
        tagged_type: Option<TaggedType>,
    ) -> Self {
        Self {
            timeframe,
            limit,
            skip,
            taggers_limit,
            tagged_type,
        }
    }
}

#[derive(Serialize, ToSchema, Deserialize)]
pub struct TaggersInfo {
    pub users: Taggers,
    pub relationship: bool,
}
