use crate::{queries, RedisOps};
use axum::async_trait;
use neo4rs::Query;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::traits::UserFollows;

#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
pub struct Following(pub Vec<String>);

impl AsRef<[String]> for Following {
    fn as_ref(&self) -> &[String] {
        &self.0
    }
}

#[async_trait]
impl RedisOps for Following {}

impl UserFollows for Following {
    fn from_vec(vec: Vec<String>) -> Self {
        Self(vec)
    }

    fn get_query(user_id: &str, skip: Option<usize>, limit: Option<usize>) -> Query {
        queries::get::get_user_following(user_id, skip, limit)
    }

    fn get_ids_field_name() -> &'static str {
        "following_ids"
    }
}
