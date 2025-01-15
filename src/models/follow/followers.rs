use crate::{queries, RedisOps};
use async_trait::async_trait;
use neo4rs::Query;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::traits::UserFollows;

#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
pub struct Followers(pub Vec<String>);

impl AsRef<[String]> for Followers {
    fn as_ref(&self) -> &[String] {
        &self.0
    }
}

#[async_trait]
impl RedisOps for Followers {}

impl UserFollows for Followers {
    fn from_vec(vec: Vec<String>) -> Self {
        Self(vec)
    }

    fn get_query(user_id: &str, skip: Option<usize>, limit: Option<usize>) -> Query {
        queries::get::get_user_followers(user_id, skip, limit)
    }

    fn get_ids_field_name() -> &'static str {
        "follower_ids"
    }
}
