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


// TODO: New implementation needed, does not work. The score list, we will get from redis. After we will query Neo4J to get the user_ids
impl HotTag {
    pub async fn get_global_tags_stream() -> Result<Option<Vec<Self>>, Box<dyn Error + Send + Sync>> {
        let hot_tags = match Self::try_from_index_sorted_set(
            &GLOBAL_HOT_TAGS,
            None, 
            None, 
            Some(0), 
            Some(20), 
            Sorting::Descending)
        .await? {
            Some(tags) => tags,
            None => return Ok(Some(Vec::new()))
        };

        // TODO: Check if this is the right way to do. If we do into_iter, we pass the ownership to do tag_list
        // and after cannot loop the hot_tags array to create a HotTag struct
        let tag_list: Vec<String> = hot_tags.iter().map(|tag| tag.0.clone()).collect();
        let query = queries::get_global_hot_tags_taggers(tag_list);
        let tag_user_list = retrieve_hot_tags_from_graph(query).await?.unwrap();

        let mut hot_tags_stream: Vec<HotTag> = Vec::with_capacity(hot_tags.len());
        for (index, (label, score)) in hot_tags.into_iter().enumerate() {
            let hot_tag = Self::new(
                label,
                tag_user_list[index].clone(),
                score as u64
            );

            hot_tags_stream.push(hot_tag);
        }
        Ok(Some(hot_tags_stream))
    }

    pub async fn get_stream_tags_from_reached(user_id: String, reach: UserStreamType) -> Result<Option<Vec<Self>>, Box<dyn Error + Send + Sync>> {
        let users = UserStream::get_user_list_from_reach(&user_id, reach, None, Some(10000)).await?;
        match users {
            Some(users) => Self::retrieve_users_tags_by_reach(&users).await,
            None => Ok(None),
        }
    }

    async fn retrieve_users_tags_by_reach(users: &[String]) -> Result<Option<Vec<Self>>, Box<dyn Error + Send + Sync>> {
        let user_slice = users.iter().map(AsRef::as_ref).collect::<Vec<&str>>();
        let query = queries::get_tags_from_user_ids(user_slice.as_slice());
        retrieve_reached_hot_tags(query).await
    }
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

async fn retrieve_reached_hot_tags(query: Query) -> Result<Option<Vec<HotTag>>, Box<dyn Error + Send + Sync>> {
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