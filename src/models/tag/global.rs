use super::stream::{TagStreamReach, Taggers};
use crate::db::graph::exec::retrieve_from_graph;
use crate::types::DynError;
use crate::{queries, RedisOps};

pub struct TagGlobal {}

impl TagGlobal {
    pub async fn get_tag_taggers(
        label: String,
        user_id: Option<String>,
        reach: Option<TagStreamReach>,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Vec<String>>, DynError> {
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
) -> Result<Option<Vec<String>>, DynError> {
    Taggers::try_from_index_set(&[label], skip, limit).await
}

pub async fn get_tag_taggers_by_reach(
    label: &str,
    user_id: &str,
    reach: TagStreamReach,
    skip: Option<usize>,
    limit: Option<usize>,
) -> Result<Option<Vec<String>>, DynError> {
    let query = queries::get::get_tag_taggers_by_reach(
        label,
        user_id,
        reach.to_graph_subquery(),
        skip,
        limit,
    );
    retrieve_from_graph::<Vec<String>>(query, "tagger_ids").await
}
