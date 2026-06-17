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

/// Web of Trust traversal depth, validated to `1..=3` at construction. The
/// `FOLLOWS*1..n` graph traversals are expensive, so the query builders only
/// accept this type, keeping the bound enforced for every caller (web, tests,
/// benches, internal) rather than at the web layer alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct WotDepth(u8);

impl Default for WotDepth {
    /// Depth-3 is expensive without caching, so default to 2.
    fn default() -> Self {
        WotDepth(2)
    }
}

impl WotDepth {
    pub const MIN: u8 = 1;
    pub const MAX: u8 = 3;

    /// Validates that `depth` is within `1..=3`.
    pub fn new(depth: u8) -> Result<Self, String> {
        if (Self::MIN..=Self::MAX).contains(&depth) {
            Ok(WotDepth(depth))
        } else {
            Err(format!(
                "'depth' must be between {} and {}",
                Self::MIN,
                Self::MAX
            ))
        }
    }

    /// The underlying depth value.
    pub fn get(self) -> u8 {
        self.0
    }
}

// Deserialize through `new` so the `1..=3` invariant holds for every input,
// not just values built at the web layer.
impl<'de> Deserialize<'de> for WotDepth {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let depth = u8::deserialize(deserializer)?;
        WotDepth::new(depth).map_err(de::Error::custom)
    }
}

impl std::fmt::Display for WotDepth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, ToSchema, Clone, PartialEq)]
pub enum StreamReach {
    Followers,
    Following,
    Friends,
    Wot(WotDepth),
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
            // Bare "wot" uses the default WoT depth (2).
            "wot" => Ok(StreamReach::Wot(WotDepth::default())),
            _ => {
                // Try to parse Wot variant with depth using wot_X format
                if let Some(depth_str) = s.strip_prefix("wot_") {
                    let depth = depth_str.parse::<u8>().map_err(|_| {
                        de::Error::custom(format!("Invalid depth value: {depth_str}"))
                    })?;
                    let depth = WotDepth::new(depth).map_err(de::Error::custom)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_wot_defaults_to_depth_2() {
        let reach: StreamReach = serde_json::from_str("\"wot\"").unwrap();
        assert_eq!(reach, StreamReach::Wot(WotDepth::default()));
        assert_eq!(WotDepth::default().get(), 2);
    }

    #[test]
    fn wot_with_explicit_depth_parses() {
        let reach: StreamReach = serde_json::from_str("\"wot_3\"").unwrap();
        assert_eq!(reach, StreamReach::Wot(WotDepth::new(3).unwrap()));
    }

    #[test]
    fn wot_out_of_range_is_rejected() {
        assert!(serde_json::from_str::<StreamReach>("\"wot_4\"").is_err());
    }
}
