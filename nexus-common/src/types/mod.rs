mod pagination;
pub mod routes;
mod timeframe;

pub use pagination::Pagination;
pub use timeframe::Timeframe;

use serde::de::{self, Deserializer};
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

#[derive(Debug, ToSchema, Clone, PartialEq)]
pub enum StreamReach {
    Followers,
    Following,
    Friends,
    Wot(u8),
}

impl<'de> Deserialize<'de> for StreamReach {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        // Handle simple variants
        match s.as_str() {
            "followers" => Ok(StreamReach::Followers),
            "following" => Ok(StreamReach::Following),
            "friends" => Ok(StreamReach::Friends),
            "wot" => Ok(StreamReach::Wot(3)), // Default to depth 3 if just "wot" is provided
            _ => {
                // Try to parse Wot variant with depth using wot_X format
                if let Some(depth_str) = s.strip_prefix("wot_") {
                    let depth = depth_str.parse::<u8>().map_err(|_| {
                        de::Error::custom(format!("Invalid depth value: {}", depth_str))
                    })?;

                    if !(1..=3).contains(&depth) {
                        return Err(de::Error::custom("Wot depth must be between 1 and 3"));
                    }

                    Ok(StreamReach::Wot(depth))
                } else {
                    Err(de::Error::unknown_variant(
                        &s,
                        &[
                            "followers",
                            "following",
                            "friends",
                            "wot",
                            "wot_1",
                            "wot_2",
                            "wot_3",
                        ],
                    ))
                }
            }
        }
    }
}
