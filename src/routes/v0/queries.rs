use crate::models::post::{PostStreamSorting, ViewerStreamSource};
use serde::de::Deserializer;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct TagsQuery {
    pub limit_tags: Option<usize>,
    pub limit_taggers: Option<usize>,
}

#[derive(Deserialize)]
pub struct PaginationQuery {
    pub skip: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Deserialize)]
pub struct PostStreamQuery {
    pub viewer_id: Option<String>,
    pub author_id: Option<String>,
    pub skip: Option<usize>,
    pub limit: Option<usize>,
    pub sorting: Option<PostStreamSorting>,
    pub source: Option<ViewerStreamSource>,
    #[serde(default, deserialize_with = "deserialize_comma_separated")]
    pub tags: Option<Vec<String>>,
    pub post_id: Option<String>,
    pub start: Option<f64>,
    pub end: Option<f64>,
}

// Custom deserializer for comma-separated tags
fn deserialize_comma_separated<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    if let Some(s) = s {
        // Split by comma and trim any excess whitespace
        let tags: Vec<String> = s.split(',').map(|tag| tag.trim().to_string()).collect();
        return Ok(Some(tags));
    }
    Ok(None)
}
