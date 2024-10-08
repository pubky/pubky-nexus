use crate::{db::graph::exec::retrieve_many_from_graph, queries, RedisOps};
use serde::{Deserialize, Serialize};
use std::error::Error;
use utoipa::ToSchema;

pub const USER_NAME_KEY_PARTS: [&str; 2] = ["Users", "Name"];

#[derive(Serialize, Deserialize, ToSchema, Default)]
pub struct UserSearch(pub Vec<String>);

impl RedisOps for UserSearch {}

impl UserSearch {
    pub async fn get_by_name(
        name: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
        let query = queries::read::search_user(name, skip, limit);
        Ok(Some(UserSearch(
            retrieve_many_from_graph::<String>(query, "id").await?,
        )))
    }
}
