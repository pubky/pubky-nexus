use super::stream::{TagStreamReach, Taggers};
use crate::types::DynError;
use crate::RedisOps;

pub struct TagGlobal {}

impl TagGlobal {
    pub async fn get_tag_taggers(
        label: String,
        reach: Option<TagStreamReach>,
    ) -> Result<Option<Vec<String>>, DynError> {
        match reach {
            None => read_from_set(&label).await,
            _ => Ok(None),
        }
    }
}

pub async fn read_from_set(label: &str) -> Result<Option<Vec<String>>, DynError> {
    Taggers::try_from_index_set(&[label], None, None).await
}
