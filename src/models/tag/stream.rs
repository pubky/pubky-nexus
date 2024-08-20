use neo4rs::Query;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use std::error::Error;

use crate::db::kv::index::sorted_sets::Sorting;
use crate::RedisOps;
use crate::{db::connectors::neo4j::get_neo4j_graph, queries};
use crate::models::user::{UserStream, UserStreamType};

pub const GLOBAL_HOT_TAGS: [&str; 3] = ["Tags", "Global", "Hot"];

#[derive(Deserialize, Serialize, ToSchema, Debug)]
pub struct HotTag {
    label: String,
    tagger_ids: Vec<String>,
    post_count: u64
}

impl HotTag {
    fn new(label: String, tagger_ids: Vec<String>, post_count: u64) -> Self {
        Self { label, tagger_ids, post_count }
    }
}

impl RedisOps for HotTag {}

type TagList = Vec<String>;



impl HotTag {
    pub async fn get_global_tags_stream(skip: Option<usize>, limit: Option<usize>) -> Result<Option<Vec<Self>>, Box<dyn Error + Send + Sync>> {
        let hot_tags = match Self::try_from_index_sorted_set(
            &GLOBAL_HOT_TAGS,
            None, 
            None, 
            skip, 
            limit, 
            Sorting::Descending
        )
        .await? {
            Some(tags) => tags,
            None => return Ok(None)
        };

        let tag_list: Vec<&str> = hot_tags.iter().map(|(label, _)| label.as_ref()).collect();
        let query = queries::get_global_hot_tags_taggers(tag_list.as_slice());
        let tag_user_list = retrieve_hot_tags_from_graph(query).await?.unwrap();

        let hot_tags_stream: Vec<HotTag> = hot_tags
            .into_iter()
            .zip(tag_user_list)
            .map(|((label, score), tagger_ids)| {
                HotTag::new(label, tagger_ids, score as u64)
            }).collect();

        Ok(Some(hot_tags_stream))
    }

    pub async fn get_stream_tags_by_reach(user_id: String, reach: UserStreamType) -> Result<Option<Vec<Self>>, Box<dyn Error + Send + Sync>> {
        // We cannot use here limit and skip because we want to get all the users reach by
        let users = UserStream::get_user_list_from_reach(&user_id, reach, None, Some(10000)).await?;
        match users {
            Some(users) => retrieve_users_tags_by_reach(&users).await,
            None => Ok(None),
        }
    }
}

async fn retrieve_users_tags_by_reach(users: &[String]) -> Result<Option<Vec<HotTag>>, Box<dyn Error + Send + Sync>> {
    let user_slice = users.iter().map(AsRef::as_ref).collect::<Vec<&str>>();
    let query = queries::get_tags_from_user_ids(user_slice.as_slice());
    retrieve_by_reach_hot_tags(query).await
}

async fn retrieve_hot_tags_from_graph(query: Query) -> Result<Option<Vec<TagList>>, Box<dyn Error + Send + Sync>> {
    let mut result;
    {
        let graph = get_neo4j_graph()?;

        let graph = graph.lock().await;
        result = graph.execute(query).await?;
    }
    if let Some(row) = result.next().await? {
        let hot_tags: Vec<TagList> = row.get("tag_user_ids")?;
        return Ok(Some(hot_tags));
    }
    Ok(None)
}

async fn retrieve_by_reach_hot_tags(query: Query) -> Result<Option<Vec<HotTag>>, Box<dyn Error + Send + Sync>> {
    let mut result;
    {
        let graph = get_neo4j_graph()?;

        let graph = graph.lock().await;
        result = graph.execute(query).await?;
    }
    if let Some(row) = result.next().await? {
        let hot_tags: Vec<HotTag> = row.get("hot_tags")?;
        return Ok(Some(hot_tags));
    }
    Ok(None)
}