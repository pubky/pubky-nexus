use crate::db::graph::exec::retrieve_from_graph;
use crate::types::DynError;
use axum::async_trait;
use chrono::Datelike;
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
}

#[derive(Deserialize, Debug, ToSchema, Clone)]
pub enum TaggedType {
    Post,
    User,
}

impl Display for TaggedType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaggedType::Post => write!(f, "Post"),
            TaggedType::User => write!(f, "User"),
        }
    }
}

#[derive(Deserialize, Debug, ToSchema, Clone)]
pub enum Timeframe {
    Today,
    ThisMonth,
    AllTime,
}

impl Display for Timeframe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Timeframe::Today => write!(f, "Today"),
            Timeframe::ThisMonth => write!(f, "ThisMonth"),
            Timeframe::AllTime => write!(f, "AllTime"),
        }
    }
}

impl Timeframe {
    pub fn to_timestamp_range(&self) -> (i64, i64) {
        let now = chrono::Utc::now();
        let start = match self {
            Timeframe::Today => now
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc()
                .timestamp_millis(),
            Timeframe::ThisMonth => now
                .date_naive()
                .with_day(1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc()
                .timestamp_millis(),
            Timeframe::AllTime => 0,
        };
        (start, now.timestamp_millis())
    }

    pub fn to_cache_period(&self) -> i64 {
        match self {
            Timeframe::Today => 60 * 60,
            Timeframe::ThisMonth => 60 * 60 * 24,
            Timeframe::AllTime => 60 * 60 * 24,
        }
    }
}

pub struct HotTagsInput {
    pub timeframe: Timeframe,
    pub skip: usize,
    pub limit: usize,
    pub taggers_limit: usize,
    pub tagged_type: Option<TaggedType>,
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
        Self::put_index_set(&[label], &[user_id], None, None).await
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
                Self::put_index_sorted_set(&TAG_GLOBAL_HOT, hot_tags_score.as_slice(), None, None)
                    .await?;
                // Add all the users_id in the SET
                for (label, user_list) in hot_tags_users.into_iter() {
                    let values_ref: Vec<&str> = user_list.iter().map(|id| id.as_str()).collect();
                    Taggers::put_index_set(&[label], &values_ref, None, None).await?;
                }
            }
        }
        Ok(())
    }

    fn get_cache_key_parts(tags_query: &HotTagsInput) -> Vec<String> {
        match &tags_query.tagged_type {
            Some(tagged) => vec![
                tags_query.timeframe.to_string(),
                tagged.to_string(),
                tags_query.limit.to_string(),
                tags_query.skip.to_string(),
            ],
            None => vec![
                tags_query.timeframe.to_string(),
                tags_query.limit.to_string(),
                tags_query.skip.to_string(),
            ],
        }
    }

    async fn get_from_cache(tags_query: &HotTagsInput) -> Result<Option<HotTags>, DynError> {
        let key_parts = HotTags::get_cache_key_parts(tags_query);
        let key_parts_vector: Vec<&str> =
            key_parts.iter().map(|s| s.as_str()).collect::<Vec<&str>>();
        HotTags::try_from_index_json(key_parts_vector.as_slice()).await
    }

    async fn set_to_cache(result: &HotTags, tags_query: &HotTagsInput) -> Result<(), DynError> {
        let key_parts = HotTags::get_cache_key_parts(tags_query);
        let key_parts_vector: Vec<&str> =
            key_parts.iter().map(|s| s.as_str()).collect::<Vec<&str>>();
        result
            .put_index_json(
                key_parts_vector.as_slice(),
                Some(tags_query.timeframe.to_cache_period()),
            )
            .await
    }

    pub async fn get_hot_tags(
        user_id: Option<String>,
        reach: Option<TagStreamReach>,
        tags_query: &HotTagsInput,
    ) -> Result<Option<HotTags>, DynError> {
        if user_id.is_none() {
            let cached_hot_tags = HotTags::get_from_cache(tags_query).await?;
            if cached_hot_tags.is_some() {
                return Ok(cached_hot_tags);
            }
        }

        let query = match &user_id {
            Some(id) => {
                queries::get::get_hot_tags_by_reach(id.as_str(), reach.unwrap(), tags_query)
            }
            None => queries::get::get_global_hot_tags(tags_query),
        };
        let result = retrieve_from_graph::<HotTags>(query, "hot_tags").await?;

        let hot_tags = result.unwrap();
        if user_id.is_none() && hot_tags.len() > 0 {
            HotTags::set_to_cache(&hot_tags, tags_query).await?;
        }

        Ok(Some(hot_tags))
    }
}
