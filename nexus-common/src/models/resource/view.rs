use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::ResourceDetails;
use crate::models::tag::TagDetails;

#[derive(Debug, Serialize, Deserialize, Default, ToSchema)]
pub struct ResourceView {
    pub details: ResourceDetails,
    pub tags: Vec<TagDetails>,
    pub taggers_count: usize,
}
