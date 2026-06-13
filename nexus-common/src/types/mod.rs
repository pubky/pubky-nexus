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

#[derive(Debug, Deserialize, Clone, PartialEq, ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(rename_all = "lowercase")]
pub enum StreamReach {
    Followers,
    Following,
    Friends,
    /// Web of Trust with default depth 3
    Wot,
    /// Web of Trust with depth 1
    #[serde(rename = "wot_1")]
    #[schema(rename = "wot_1")]
    Wot1,
    /// Web of Trust with depth 2
    #[serde(rename = "wot_2")]
    #[schema(rename = "wot_2")]
    Wot2,
    /// Web of Trust with depth 3
    #[serde(rename = "wot_3")]
    #[schema(rename = "wot_3")]
    Wot3,
}

impl StreamReach {
    /// Returns the WoT depth for Wot variants, used in graph queries.
    pub fn wot_depth(&self) -> Option<u8> {
        match self {
            StreamReach::Wot | StreamReach::Wot3 => Some(3),
            StreamReach::Wot1 => Some(1),
            StreamReach::Wot2 => Some(2),
            _ => None,
        }
    }
}
