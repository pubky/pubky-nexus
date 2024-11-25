use crate::types::DynError;
use axum::async_trait;
use neo4rs::Query;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::ops::Deref;
use utoipa::ToSchema;

use crate::{db::connectors::neo4j::get_neo4j_graph, queries};
use crate::{RedisOps, ScoreAction};

pub const TAG_GLOBAL_HOT: [&str; 3] = ["Tags", "Global", "Hot"];

#[derive(Serialize, Deserialize, Debug, ToSchema)]
pub struct Taggers(pub Vec<String>);

#[derive(Deserialize, Debug, ToSchema, Clone)]
#[serde(rename_all = "snake_case")]
pub enum TagStreamReach {
    Followers,
    Following,
    Friends,
    MostFollowed,
    Influencers,
}

impl TagStreamReach {
    pub fn to_graph_subquery(&self) -> String {
        let query = match self {
            TagStreamReach::Followers => "MATCH (user:User)<-[:FOLLOWS]-(reach:User)",
            TagStreamReach::Following => "MATCH (user:User)-[:FOLLOWS]->(reach:User)",
            TagStreamReach::Friends => {
                "MATCH (user:User)-[:FOLLOWS]->(reach:User), (user)<-[:FOLLOWS]-(reach)"
            }
            TagStreamReach::MostFollowed => "",
            TagStreamReach::Influencers => "",
        };
        String::from(query)
    }
}

#[derive(Deserialize, Debug, ToSchema, Clone)]
pub enum TaggedType {
    Post,
    User,
}

pub struct HotTagsInput {
    pub from: i64,
    pub to: i64,
    pub skip: usize,
    pub limit: usize,
    pub taggers_limit: usize,
    pub tagged_type: Option<TaggedType>,
}

impl Display for TaggedType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaggedType::Post => write!(f, "Post"),
            TaggedType::User => write!(f, "User"),
        }
    }
}

#[async_trait]
impl RedisOps for Taggers {
    async fn prefix() -> String {
        String::from("Tags:Taggers")
    }
}

impl AsRef<[String]> for Taggers {
    fn as_ref(&self) -> &[String] {
        &self.0
    }
}

impl Taggers {
    pub async fn update_index_score(
        label: &str,
        score_action: ScoreAction,
    ) -> Result<(), DynError> {
        Self::put_score_index_sorted_set(&TAG_GLOBAL_HOT, &[label], score_action).await
    }

    pub async fn put_to_index(label: &str, user_id: &str) -> Result<(), DynError> {
        Self::put_index_set(&[label], &[user_id]).await
    }

    pub async fn del_from_index(&self, label: &str) -> Result<(), DynError> {
        self.remove_from_index_set(&[label]).await
    }
}

#[derive(Deserialize, Serialize, ToSchema, Debug)]
pub struct HotTag {
    label: String,
    taggers_id: Taggers,
    tagged_count: u64,
    taggers_count: usize,
}

// Define a newtype wrapper
#[derive(Serialize, Deserialize, Debug, ToSchema, Default)]
pub struct HotTags(pub Vec<HotTag>);

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
    pub async fn set_global_tag_scores() -> Result<(), DynError> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let graph = graph.lock().await;

            let query = queries::get::get_global_hot_tags_scores();
            result = graph.execute(query).await?;
        }

        if let Some(row) = result.next().await? {
            let hot_tags_score: Vec<(f64, &str)> = row.get("hot_tags_score").unwrap_or(Vec::new());
            let hot_tags_users: Vec<(&str, Vec<String>)> =
                row.get("hot_tags_users").unwrap_or(Vec::new());
            // Make sure both list has content before write the indexes
            if !hot_tags_score.is_empty() && !hot_tags_users.is_empty() {
                Self::put_index_sorted_set(&TAG_GLOBAL_HOT, hot_tags_score.as_slice()).await?;
                // Add all the users_id in the SET
                for (label, user_list) in hot_tags_users.into_iter() {
                    let values_ref: Vec<&str> = user_list.iter().map(|id| id.as_str()).collect();
                    Taggers::put_index_set(&[label], &values_ref).await?;
                }
            }
        }
        Ok(())
    }

    pub async fn get_global_hot_tags(tags_query: &HotTagsInput) -> Result<Option<Self>, DynError> {
        let query = queries::get::get_global_hot_tags(tags_query);
        retrieve_from_graph::<HotTags>(query, "hot_tags").await
    }

    pub async fn get_hot_tags_by_reach(
        user_id: String,
        reach: TagStreamReach,
        tags_query: &HotTagsInput,
    ) -> Result<Option<HotTags>, DynError> {
        let query =
            queries::get::get_hot_tags_by_reach(&user_id, reach.to_graph_subquery(), tags_query);
        retrieve_from_graph::<HotTags>(query, "hot_tags").await
    }
}

// Generic function to retrieve data from Neo4J
async fn retrieve_from_graph<T>(query: Query, key: &str) -> Result<Option<T>, DynError>
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
