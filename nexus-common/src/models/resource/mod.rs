pub mod stream;
pub mod tag;
pub mod view;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct ResourceDetails {
    pub id: String,
    pub uri: String,
    pub scheme: String,
    pub indexed_at: i64,
}

impl ResourceDetails {
    /// Load a Resource node's details from Neo4j by resource_id.
    pub async fn get_by_id(resource_id: &str) -> crate::models::error::ModelResult<Option<Self>> {
        let query = crate::db::queries::get::get_resource_by_id(resource_id);
        let maybe_row = crate::db::fetch_row_from_graph(query).await?;
        Ok(maybe_row.map(|row| Self {
            id: row.get("id").unwrap_or_default(),
            uri: row.get("uri").unwrap_or_default(),
            scheme: row.get("scheme").unwrap_or_default(),
            indexed_at: row.get("indexed_at").unwrap_or(0),
        }))
    }
}
