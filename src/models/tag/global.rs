use super::stream::Taggers;
use crate::{models::user::UserStreamType, RedisOps};
use std::error::Error;

pub struct Global {}

impl Global {
    pub async fn get_tag_taggers(
        label: String,
        reach: Option<UserStreamType>,
    ) -> Result<Option<Vec<String>>, Box<dyn std::error::Error + Send + Sync>> {
        match reach {
            None => read_from_set(&label).await,
            _ => Ok(None),
        }
    }
}

pub async fn read_from_set(
    label: &str,
) -> Result<Option<Vec<String>>, Box<dyn Error + Send + Sync>> {
    Taggers::get_single_set(&[label], None).await
}
