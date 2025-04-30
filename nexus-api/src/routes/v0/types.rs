use nexus_common::models::tag::Taggers;
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Default, Deserialize, Debug, ToSchema)]
pub struct TagsQuery {
    pub limit_tags: Option<usize>,
    pub skip_tags: Option<usize>,
    pub limit_taggers: Option<usize>,
    pub viewer_id: Option<String>,
    #[serde(default, deserialize_with = "parse_string_to_u8")]
    pub depth: Option<u8>,
}

// Parsing strings or floats into f64
fn parse_string_to_u8<'de, D>(deserializer: D) -> Result<Option<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        Some(s) => s.parse::<u8>().map(Some).map_err(de::Error::custom),
        None => Ok(None),
    }
}

// DTO (Data Transfer Object) is used to transfer structured data between API layers,
// ensuring clear separation between internal models and external representations
#[derive(Serialize, ToSchema, Deserialize)]
pub struct TaggersInfoResponse {
    pub users: Taggers,
    pub relationship: bool,
}

impl From<(Taggers, bool)> for TaggersInfoResponse {
    fn from(tuple: (Taggers, bool)) -> Self {
        Self {
            users: tuple.0,
            relationship: tuple.1,
        }
    }
}
