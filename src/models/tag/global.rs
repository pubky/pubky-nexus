use super::stream::Taggers;
use crate::{get_neo4j_graph, models::user::UserStreamType, queries, RedisOps};
use std::error::Error;

pub struct TagGlobal {}

impl TagGlobal {
    pub async fn get_tag_taggers(
        label: String,
        user_id: Option<String>,
        reach: Option<UserStreamType>,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Vec<String>>, Box<dyn std::error::Error + Send + Sync>> {
        match user_id {
            None => read_from_set(&label, skip, limit).await,
            Some(id) => get_tag_taggers_by_reach(&label, &id, reach.unwrap(), skip, limit).await,
        }
    }
}

pub async fn read_from_set(
    label: &str,
    skip: Option<usize>,
    limit: Option<usize>,
) -> Result<Option<Vec<String>>, Box<dyn Error + Send + Sync>> {
    Taggers::try_from_index_set(&[label], skip, limit).await
}

pub async fn get_tag_taggers_by_reach(
    label: &str,
    user_id: &str,
    reach: UserStreamType,
    skip: Option<usize>,
    limit: Option<usize>,
) -> Result<Option<Vec<String>>, Box<dyn std::error::Error + Send + Sync>> {
    let graph = get_neo4j_graph()?;
    let graph = graph.lock().await;

    let query = queries::read::get_tag_taggers_by_reach(
        label,
        user_id,
        reach.to_graph_subquery(),
        skip,
        limit,
    );
    let mut result = graph.execute(query).await?;

    let mut tagger_ids: Vec<String> = vec![];
    while let Some(row) = result.next().await? {
        if let Some(id) = row.get("id")? {
            tagger_ids.push(id)
        };
    }

    Ok(Some(tagger_ids))
}
