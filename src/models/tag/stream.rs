use neo4rs::Query;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::ops::Deref;
use utoipa::ToSchema;

use crate::db::kv::index::sorted_sets::Sorting;
use crate::models::user::{UserStream, UserStreamType};
use crate::RedisOps;
use crate::{db::connectors::neo4j::get_neo4j_graph, queries};

pub const TAG_GLOBAL_HOT: [&str; 3] = ["Tags", "Global", "Hot"];

type TagList = Vec<String>;

#[derive(Deserialize, Serialize, ToSchema, Debug)]
pub struct HotTag {
    label: String,
    tagger_ids: Vec<String>,
    post_count: u64,
}

// Define a newtype wrapper
#[derive(Serialize, Deserialize, Debug, ToSchema, Default)]
pub struct HotTags(Vec<HotTag>);

impl RedisOps for HotTags {}

// Implement Deref so TagList can be used like Vec<String>
impl Deref for HotTags {
    type Target = Vec<HotTag>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// Create a HotTags instance directly from an iterator of HotTag items
// Need it in collect()
impl FromIterator<HotTag> for HotTags {
    fn from_iter<I: IntoIterator<Item = HotTag>>(iter: I) -> Self {
        HotTags(iter.into_iter().collect())
    }
}

impl HotTags {
    // TODO: Move another struct that is more related with reindexer process?
    pub async fn set_global_tag_scores(
        hot_tag_threshold: u64,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let graph = graph.lock().await;

            let query = queries::get_global_hot_tags_scores(hot_tag_threshold);
            result = graph.execute(query).await?;
        }

        if let Some(row) = result.next().await? {
            let hot_tags: Vec<(f64, &str)> = row.get("hot_tags")?;
            Self::put_index_sorted_set(&TAG_GLOBAL_HOT, hot_tags.as_slice()).await?
        }
        Ok(())
    }
    pub async fn get_global_tags_stream(
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
        let hot_tags = Self::try_from_index_sorted_set(
            &TAG_GLOBAL_HOT,
            None,
            None,
            skip,
            limit,
            Sorting::Descending,
        )
        .await?
        .unwrap_or_default();

        if hot_tags.is_empty() {
            return Ok(None);
        }

        let tag_list: Vec<&str> = hot_tags.iter().map(|(label, _)| label.as_ref()).collect();
        let query = queries::get_global_hot_tags_taggers(tag_list.as_slice());
        let tag_user_list = retrieve_from_graph::<Vec<TagList>>(query, "tag_user_ids")
            .await?
            .unwrap();

        let hot_tags_stream: HotTags = hot_tags
            .into_iter()
            .zip(tag_user_list)
            .map(|((label, score), tagger_ids)| HotTag {
                label,
                tagger_ids,
                post_count: score as u64,
            })
            .collect();

        Ok(Some(hot_tags_stream))
    }

    pub async fn get_stream_tags_by_reach(
        user_id: String,
        reach: UserStreamType,
    ) -> Result<Option<HotTags>, Box<dyn Error + Send + Sync>> {
        // We cannot use here limit and skip because we want to get all the users reach by
        let users =
            UserStream::get_user_list_from_reach(&user_id, reach, None, Some(10000)).await?;
        match users {
            Some(users) => get_users_tags_by_reach(&users).await,
            None => Ok(None),
        }
    }
}

async fn get_users_tags_by_reach(
    users: &[String],
) -> Result<Option<HotTags>, Box<dyn Error + Send + Sync>> {
    let user_slice = users.iter().map(AsRef::as_ref).collect::<Vec<&str>>();
    let query = queries::get_tags_by_user_ids(user_slice.as_slice());
    retrieve_from_graph::<HotTags>(query, "hot_tags").await
}

// Generic function to retrieve data from Neo4J
async fn retrieve_from_graph<T>(
    query: Query,
    key: &str,
) -> Result<Option<T>, Box<dyn Error + Send + Sync>>
where
    // Key point: DeserializeOwned ensures we can deserialize into any type that implements it
    T: DeserializeOwned + Send + Sync,
{
    let mut result;
    {
        let graph = get_neo4j_graph()?;
        let graph = graph.lock().await;
        result = graph.execute(query).await?;
    }

    if let Some(row) = result.next().await? {
        let data: T = row.get(key)?;
        return Ok(Some(data));
    }

    Ok(None)
}
