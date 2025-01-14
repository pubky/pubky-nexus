use crate::db::graph::exec::retrieve_from_graph;
use crate::db::kv::index::sorted_sets::SortOrder;
use crate::types::DynError;
use crate::types::StreamReach;
use crate::types::Timeframe;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use utoipa::ToSchema;

use crate::queries;
use crate::RedisOps;

const GLOBAL_INFLUENCERS_PREFIX: &str = "Cache:Influencers";

#[derive(Deserialize, Serialize, ToSchema, Debug, Clone)]
pub struct Influencer {
    pub id: String,
    pub score: f64,
}

// Define a newtype wrapper
#[derive(Serialize, Deserialize, Debug, ToSchema, Default, Clone)]
pub struct Influencers(pub Vec<Influencer>);

impl RedisOps for Influencers {}

// Create a Influencers instance directly from an iterator of Influencer items
// Need it in collect()
impl FromIterator<Influencer> for Influencers {
    fn from_iter<I: IntoIterator<Item = Influencer>>(iter: I) -> Self {
        Influencers(iter.into_iter().collect())
    }
}

// Implement Deref so Influencers can be used like Vec<String>
impl Deref for Influencers {
    type Target = Vec<Influencer>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Influencers {
    pub async fn reindex() -> Result<(), DynError> {
        Influencers::get_global_influencers(0, 100, &Timeframe::AllTime).await?;
        Influencers::get_global_influencers(0, 100, &Timeframe::ThisMonth).await?;
        Ok(())
    }

    fn get_cache_key_parts(timeframe: &Timeframe) -> Vec<String> {
        vec![timeframe.to_string()]
    }

    async fn get_from_global_cache(
        skip: usize,
        limit: usize,
        timeframe: &Timeframe,
    ) -> Result<Option<Influencers>, DynError> {
        let key_parts = Influencers::get_cache_key_parts(timeframe);
        let key_parts_vector: Vec<&str> =
            key_parts.iter().map(|s| s.as_str()).collect::<Vec<&str>>();
        let ranking = Influencers::try_from_index_sorted_set(
            key_parts_vector.as_slice(),
            None,
            None,
            Some(skip),
            Some(limit),
            SortOrder::Descending,
            Some(GLOBAL_INFLUENCERS_PREFIX),
        )
        .await?;

        match ranking {
            None => Ok(None),
            Some(ranking) => {
                let mut influencers = Vec::new();
                for (id, score) in ranking {
                    influencers.push(Influencer { id, score });
                }
                Ok(Some(Influencers(influencers)))
            }
        }
    }

    async fn put_to_global_cache(
        result: Influencers,
        timeframe: &Timeframe,
    ) -> Result<(), DynError> {
        let key_parts = Influencers::get_cache_key_parts(timeframe);
        let key_parts_vector: Vec<&str> =
            key_parts.iter().map(|s| s.as_str()).collect::<Vec<&str>>();

        // store the ranking as sorted set in cache
        Influencers::put_index_sorted_set(
            key_parts_vector.as_slice(),
            result
                .iter()
                .map(|influencer| (influencer.score, influencer.id.as_str()))
                .collect::<Vec<(f64, &str)>>()
                .as_slice(),
            Some(GLOBAL_INFLUENCERS_PREFIX),
            Some(timeframe.to_cache_period()),
        )
        .await?;
        Ok(())
    }

    pub async fn get_influencers(
        user_id: Option<&str>,
        reach: Option<StreamReach>,
        skip: usize,
        limit: usize,
        timeframe: &Timeframe,
        preview: bool,
    ) -> Result<Option<Influencers>, DynError> {
        let (skip, limit) = if preview {
            let skip = Utc::now().timestamp_subsec_micros() % 98;
            (skip as usize, 3)
        } else {
            (skip, limit)
        };
        match user_id {
            Some(user_id) => {
                Influencers::get_influencers_by_reach(
                    user_id,
                    reach.unwrap_or(StreamReach::Friends),
                    skip,
                    limit,
                    timeframe,
                )
                .await
            }
            None => Influencers::get_global_influencers(skip, limit, timeframe).await,
        }
    }

    async fn get_influencers_by_reach(
        user_id: &str,
        reach: StreamReach,
        skip: usize,
        limit: usize,
        timeframe: &Timeframe,
    ) -> Result<Option<Influencers>, DynError> {
        let query = queries::get::get_influencers_by_reach(user_id, reach, skip, limit, timeframe);
        retrieve_from_graph::<Influencers>(query, "influencers").await
    }

    async fn get_global_influencers(
        skip: usize,
        limit: usize,
        timeframe: &Timeframe,
    ) -> Result<Option<Influencers>, DynError> {
        let cached_influencers = Influencers::get_from_global_cache(skip, limit, timeframe).await?;
        if cached_influencers.is_some() {
            return Ok(cached_influencers);
        }

        let query = queries::get::get_global_influencers(0, 100, timeframe);
        let result = retrieve_from_graph::<Influencers>(query, "influencers").await?;

        let influencers = match result {
            Some(influencers) => influencers,
            None => return Ok(None),
        };
        if influencers.len() > 0 {
            Influencers::put_to_global_cache(influencers.clone(), timeframe).await?;
        }

        Influencers::get_from_global_cache(skip, limit, timeframe).await
    }
}
