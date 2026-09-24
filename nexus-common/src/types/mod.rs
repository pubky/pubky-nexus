mod pagination;
pub mod routes;
mod timeframe;

pub use pagination::Pagination;
pub use timeframe::{CacheTimeframe, Timeframe};

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

/// Trust set for a `wot_domain` post stream. `Me` is the observer's own
/// `TAGGED` edges (depth-0, no follow traversal); `Network(d)` is the taggers
/// reached via `FOLLOWS*1..d`. Keeping `Me` on this enum (rather than widening
/// `WotDepth`) makes depth-0 unrepresentable for `source=wot` and `reach=wot_N`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum DomainTrust {
    Me,
    Network(WotDepth),
}

/// Every string [`StreamReach`] deserializes from.
const STREAM_REACH_VALUES: [&str; 7] = [
    "followers",
    "following",
    "friends",
    "wot",
    "wot_1",
    "wot_2",
    "wot_3",
];

/// A user's reach, as the query string spells it: `followers`, `following`,
/// `friends`, `wot` (depth 2) or `wot_1`..`wot_3`.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamReach {
    Followers,
    Following,
    Friends,
    Wot(WotDepth),
}

impl StreamReach {
    /// Low-cardinality reach value and optional WoT depth for telemetry.
    pub(crate) fn telemetry_dimensions(&self) -> (&'static str, Option<u8>) {
        match self {
            StreamReach::Followers => ("followers", None),
            StreamReach::Following => ("following", None),
            StreamReach::Friends => ("friends", None),
            StreamReach::Wot(depth) => ("wot", Some(depth.get())),
        }
    }
}

// Documents the string form the deserializer accepts; a derived schema would
// describe the Rust enum (`{"Wot": 2}`) instead.
impl utoipa::PartialSchema for StreamReach {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        use utoipa::openapi::schema::{ObjectBuilder, SchemaType, Type};
        ObjectBuilder::new()
            .schema_type(SchemaType::new(Type::String))
            .enum_values(Some(STREAM_REACH_VALUES))
            .description(Some(
                "Reach of a user. Bare `wot` is the web of trust at depth 2, `wot_1`..`wot_3` set the depth.",
            ))
            .examples([serde_json::json!("wot_2")])
            .into()
    }
}

impl ToSchema for StreamReach {}

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
                    Err(de::Error::unknown_variant(&s, &STREAM_REACH_VALUES))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_documents_exactly_the_accepted_strings() {
        use utoipa::openapi::RefOr;
        let schema = match <StreamReach as utoipa::PartialSchema>::schema() {
            RefOr::T(schema) => serde_json::to_value(schema).unwrap(),
            RefOr::Ref(_) => panic!("StreamReach schema must be inline"),
        };
        assert_eq!(schema["type"], "string");
        let values: Vec<&str> = schema["enum"]
            .as_array()
            .expect("enum values")
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(values, STREAM_REACH_VALUES);
        for value in values {
            serde_json::from_value::<StreamReach>(serde_json::json!(value))
                .unwrap_or_else(|e| panic!("documented value {value} must parse: {e}"));
        }
    }

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
