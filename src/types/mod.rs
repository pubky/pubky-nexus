mod pagination;
mod pubky;

pub use pagination::Pagination;
pub use pubky::PubkyId;

use serde::{Deserialize, Serialize};
use std::error::Error;
use utoipa::ToSchema;

pub type DynError = Box<dyn Error + Send + Sync>;

#[derive(Debug, Serialize, Deserialize, ToSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StreamSorting {
    Timeline,
    TotalEngagement,
}
