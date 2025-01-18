use crate::{db::graph::exec::retrieve_from_graph, queries, types::DynError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use utoipa::ToSchema;

use crate::{RedisOps, ScoreAction};

use super::{stream::TagStreamReach, traits::taggers::Taggers as TaggersType};

// DELETE: LEGACY
pub const TAG_GLOBAL_HOT: [&str; 3] = ["Tags", "Global", "Hot"];

#[derive(Serialize, Deserialize, Debug, ToSchema, Clone)]
pub struct Taggers(pub TaggersType);

impl Deref for Taggers {
    type Target = TaggersType;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[async_trait]
impl RedisOps for Taggers {
    // DELETE: LEGACY
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
    // DELETE: LEGACY
    pub async fn update_index_score(
        label: &str,
        score_action: ScoreAction,
    ) -> Result<(), DynError> {
        Self::put_score_index_sorted_set(&TAG_GLOBAL_HOT, &[label], score_action).await
    }
    // DELETE: LEGACY
    pub async fn put_to_index(label: &str, user_id: &str) -> Result<(), DynError> {
        Self::put_index_set(&[label], &[user_id], None, None).await
    }
    // DELETE: LEGACY
    pub async fn del_from_index(&self, label: &str) -> Result<(), DynError> {
        self.remove_from_index_set(&[label]).await
    }

    pub async fn get_global_taggers(
        label: String,
        user_id: Option<String>,
        reach: Option<TagStreamReach>,
        skip: usize,
        limit: usize,
    ) -> Result<Option<TaggersType>, DynError> {
        match user_id {
            None => Self::read_from_set(&label, Some(skip), Some(limit)).await,
            Some(id) => {
                Self::get_tag_taggers_by_reach(
                    &label,
                    &id,
                    reach.unwrap_or(TagStreamReach::Friends),
                    skip,
                    limit,
                )
                .await
            }
        }
    }

    async fn read_from_set(
        label: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<TaggersType>, DynError> {
        Taggers::try_from_index_set(&[label], skip, limit, None).await
    }

    async fn get_tag_taggers_by_reach(
        label: &str,
        user_id: &str,
        reach: TagStreamReach,
        skip: usize,
        limit: usize,
    ) -> Result<Option<TaggersType>, DynError> {
        let query = queries::get::get_tag_taggers_by_reach(label, user_id, reach, skip, limit);
        retrieve_from_graph::<TaggersType>(query, "tagger_ids").await
    }
}
