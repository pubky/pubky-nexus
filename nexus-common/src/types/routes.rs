use super::Timeframe;
use crate::models::tag::TaggedType;

pub struct HotTagsInputDTO {
    pub timeframe: Timeframe,
    pub skip: usize,
    pub limit: usize,
    pub taggers_limit: usize,
    pub tagged_type: Option<TaggedType>,
}

impl HotTagsInputDTO {
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
