mod pagination;
pub mod routes;
mod timeframe;

pub use pagination::Pagination;
pub use timeframe::Timeframe;

use serde::{Deserialize, Serialize};
use std::error::Error;
use utoipa::ToSchema;

pub type DynError = Box<dyn Error + Send + Sync>;

#[derive(Debug, Serialize, Deserialize, ToSchema, PartialEq, Default, Clone)]
#[serde(rename_all = "snake_case")]
pub enum StreamSorting {
    #[default]
    Timeline,
    TotalEngagement,
}

#[derive(Deserialize, Debug, ToSchema, Clone)]
#[serde(rename_all = "snake_case")]
pub enum StreamReach {
    Followers,
    Following,
    Friends,
    Wot(u8),
}
