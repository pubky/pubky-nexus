use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::db::fetch_row_from_graph;
use crate::db::queries;
use crate::models::error::ModelError;
use crate::models::error::ModelResult;

/// Represents a Pubky tag with uri, label, indexed at timestamp.
#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
pub struct TagView {
    pub uri: String,
    pub label: String,
    pub indexed_at: i64,
}

impl TagView {
    /// Retrieves a user by ID, checking the cache first and then the graph database.
    pub async fn get_by_tagger_and_id(tagger_id: &str, tag_id: &str) -> ModelResult<Option<Self>> {
        let query = queries::get::get_tag_by_tagger_and_id(tagger_id, tag_id);
        let result = fetch_row_from_graph(query).await?;

        let Some(row) = result else {
            return Ok(None);
        };

        let tagged_labels: Vec<String> = row.get("tagged_labels")?;
        let tagged_uri: Option<String> = row.get("tagged_uri")?;
        let uri = if tagged_labels.iter().any(|label| label == "Post") {
            tagged_uri.ok_or_else(|| ModelError::from_generic("Tagged post has no uri"))?
        } else if tagged_labels.iter().any(|label| label == "User") {
            tagged_uri.unwrap_or_default()
        } else {
            return Err(ModelError::from_generic(format!(
                "Tagged resource has unsupported labels: {:?}",
                tagged_labels
            )));
        };

        Ok(Some(Self {
            uri,
            label: row.get("label")?,
            indexed_at: row.get("indexed_at")?,
        }))
    }
}
