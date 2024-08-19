use neo4rs::Query;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use std::error::Error;

use crate::{db::connectors::neo4j::get_neo4j_graph, queries};
use crate::models::user::{UserStream, UserStreamType};

#[derive(Deserialize, Serialize, ToSchema, Debug)]
pub struct StreamTags {
    times: u64,
    label: String,
    tagger_ids: Vec<String>,
    post_count: u64
}

impl StreamTags {
    pub async fn get_global_tags_stream() -> Result<Option<Vec<Self>>, Box<dyn Error + Send + Sync>>{
        let query = queries::get_global_hot_tags();
        Self::retrieve_hot_tags(query).await
    }

    pub async fn get_stream_tags_from_reached(user_id: String, reach: UserStreamType) -> Result<Option<Vec<Self>>, Box<dyn Error + Send + Sync>> {
        let users = UserStream::get_user_list_from_reach(&user_id, reach, None, Some(10000)).await?;
        match users {
            Some(users) => Self::retrieve_users_tags_by_reach(&users).await,
            None => Ok(None),
        }
    }

    async fn retrieve_users_tags_by_reach(users: &Vec<String>) -> Result<Option<Vec<Self>>, Box<dyn Error + Send + Sync>> {
        let user_slice = users.iter().map(AsRef::as_ref).collect::<Vec<&str>>();
        let query = queries::get_tags_from_user_ids(user_slice.as_slice());
        Self::retrieve_hot_tags(query).await
    }

    async fn retrieve_hot_tags(query: Query) -> Result<Option<Vec<Self>>, Box<dyn Error + Send + Sync>>{
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