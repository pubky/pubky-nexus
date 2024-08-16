use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use std::error::Error;

use crate::{db::connectors::neo4j::get_neo4j_graph, queries};

// TODO: Merge with PostStreamReach
#[derive(Deserialize, ToSchema)]
pub enum StreamReach {
    Following,
    Followers,
    Friends
}

#[derive(Deserialize, Serialize, ToSchema, Debug)]
pub struct StreamTags {
    times: u64,
    label: String,
    user_ids: Vec<String>
}

impl StreamTags {
    pub async fn get_global_tags_stream() -> Result<Option<Vec<Self>>, Box<dyn Error + Send + Sync>>{
        let query = queries::get_global_hot_tags();
        let mut result;
        {
            let graph = get_neo4j_graph()?;

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }
        if let Some(row) = result.next().await? {
            let hot_tags: Vec<StreamTags> = row.get("hot_tags")?;
            return Ok(Some(hot_tags));
        }
        Ok(None)
    }
}