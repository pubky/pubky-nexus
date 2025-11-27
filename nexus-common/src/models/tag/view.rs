use pubky_app_specs::{post_uri_builder, user_uri_builder};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::db::fetch_row_from_graph;
use crate::db::queries;

use crate::types::DynError;

/// Represents a Pubky tag with uri, label, indexed at timestamp.
#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
pub struct TagView {
    pub uri: String,
    pub label: String,
    pub indexed_at: i64,
}

impl TagView {
    /// Retrieves a user by ID, checking the cache first and then the graph database.
    pub async fn get_by_tagger_and_id(
        tagger_id: &str,
        tag_id: &str,
    ) -> Result<Option<Self>, DynError> {
        let query = queries::get::get_tag_by_tagger_and_id(tagger_id, tag_id);
        let result = fetch_row_from_graph(query).await?;

        let Some(row) = result else {
            return Ok(None);
        };

        let tagged_labels: Vec<String> = row.get("tagged_labels")?;
        let tagged_id = row.get("tagged_id")?;
        let uri = if tagged_labels.iter().any(|label| label == "Post") {
            let Some(author_id) = row.get("author_id")? else {
                return Err("Tagged post missing author id".into());
            };
            post_uri_builder(author_id, tagged_id)
        } else if tagged_labels.iter().any(|label| label == "User") {
            user_uri_builder(tagged_id)
        } else {
            return Err(format!(
                "Tagged resource has unsupported labels: {:?}",
                tagged_labels
            )
            .into());
        };

        Ok(Some(Self {
            uri,
            label: row.get("label")?,
            indexed_at: row.get("indexed_at")?,
        }))
    }
}
