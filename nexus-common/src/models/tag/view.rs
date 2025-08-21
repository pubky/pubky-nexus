use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::db::fetch_row_from_graph;
use crate::db::queries;

use crate::types::DynError;

/// Represents a Pubky tag with uri, label, indexed at timestamp.
#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
pub struct TagView {
    pub tag_uri: String,
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

        Ok(Some(Self {
            tag_uri: format!("pubky://{}/pub/pubky.app/tags/{}", tagger_id, tag_id),
            label: row.get("label")?,
            indexed_at: row.get("indexed_at")?,
        }))
    }
}
