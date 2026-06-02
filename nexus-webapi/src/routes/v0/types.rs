use crate::models::PubkyId;
use crate::{Error, Result as AppResult};
use nexus_common::models::tag::Taggers;
use nexus_common::types::WotDepth;
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Resolves the WoT depth for tag endpoints: `Some(depth)` activates WoT filtering,
/// `None` is the global path. Rejects an out-of-range depth requested together with a
/// viewer (an explicit but malformed WoT request); `depth` without a viewer is ignored.
pub(crate) fn resolve_tag_wot_depth(
    viewer_id: Option<&str>,
    depth: Option<u8>,
) -> AppResult<Option<WotDepth>> {
    match (viewer_id, depth) {
        (Some(_), Some(d)) => WotDepth::new(d)
            .map(Some)
            .map_err(|e| Error::invalid_input(&e)),
        _ => Ok(None),
    }
}

#[derive(Default, Deserialize, Debug, ToSchema)]
pub struct TagsQuery {
    pub limit_tags: Option<usize>,
    pub skip_tags: Option<usize>,
    pub limit_taggers: Option<usize>,
    pub viewer_id: Option<PubkyId>,
    #[serde(default, deserialize_with = "parse_string_to_u8")]
    pub depth: Option<u8>,
}

// Query params arrive as strings, so deserialize via String first.
pub(crate) fn parse_string_to_u8<'de, D>(deserializer: D) -> Result<Option<u8>, D::Error>
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
