use crate::models::post::{PostStreamSorting, ViewerStreamSource};
use crate::routes::v0::queries::PaginationQuery;
use serde::de::Deserializer;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Filters {
    pub source: Option<ViewerStreamSource>,
    #[serde(default, deserialize_with = "deserialize_comma_separated")]
    pub tags: Option<Vec<String>>,
    pub author_id: Option<String>,
    pub post_id: Option<String>,
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

#[derive(Deserialize)]
pub struct PostStreamQuery {
    pub viewer_id: Option<String>,
    pub sorting: Option<PostStreamSorting>,
    #[serde(flatten)]
    pub filters: Filters,
    #[serde(flatten)]
    pub pagination: PaginationQuery,
}

impl PostStreamQuery {
    pub fn initialize_defaults(&mut self) {
        self.pagination.skip.get_or_insert(0);
        self.pagination.limit = Some(self.pagination.limit.unwrap_or(10).min(30));
        self.sorting.get_or_insert(PostStreamSorting::Timeline);
        self.filters.source.get_or_insert(ViewerStreamSource::All);
    }
}
