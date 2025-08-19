use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::db::get_neo4j_graph;
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
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query = queries::get::get_tag_by_tagger_and_id(tagger_id, tag_id);

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        match result.next().await? {
            Some(row) => Ok(Some(Self {
                tag_uri: format!("pubky://{}/pub/pubky.app/tags/{}", tagger_id, tag_id),
                label: row.get("label")?,
                indexed_at: row.get("indexed_at")?,
            })),
            None => Ok(None),
        }
    }
}
