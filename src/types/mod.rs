mod pagination;
mod pubky;
mod timeframe;

pub use pagination::Pagination;
pub use pubky::PubkyId;
pub use timeframe::Timeframe;

use serde::{Deserialize, Serialize};
use std::error::Error;
use utoipa::ToSchema;

pub type DynError = Box<dyn Error + Send + Sync>;

#[derive(Debug, Serialize, Deserialize, ToSchema, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum StreamSorting {
    #[default]
    Timeline,
    TotalEngagement,
}
