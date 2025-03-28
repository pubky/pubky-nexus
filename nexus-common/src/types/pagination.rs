use crate::db::kv::SortOrder;
use serde::de::{self, Deserializer};
use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Default, Deserialize, Debug, ToSchema)]
pub struct Pagination {
    #[serde(default, deserialize_with = "parse_string_to_usize")]
    pub skip: Option<usize>,
    #[serde(default, deserialize_with = "parse_string_to_usize")]
    pub limit: Option<usize>,
    #[serde(default, deserialize_with = "parse_string_to_f64")]
    pub start: Option<f64>,
    #[serde(default, deserialize_with = "parse_string_to_f64")]
    pub end: Option<f64>,
    pub order: Option<SortOrder>,
}

// Parse a string into a usize
fn parse_string_to_usize<'de, D>(deserializer: D) -> Result<Option<usize>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        Some(s) => s.parse::<usize>().map(Some).map_err(de::Error::custom),
        None => Ok(None),
    }
}

// Parsing strings or floats into f64
fn parse_string_to_f64<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        Some(s) => s.parse::<f64>().map(Some).map_err(de::Error::custom),
        None => Ok(None),
    }
}
