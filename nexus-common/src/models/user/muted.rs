use crate::types::DynError;
use crate::db::{queries, RedisOps, execute_graph_operation, OperationOutcome, get_neo4j_graph};
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
pub struct Muted(pub Vec<String>);

impl AsRef<[String]> for Muted {
    fn as_ref(&self) -> &[String] {
        &self.0
    }
}

#[async_trait]
impl RedisOps for Muted {}

impl Muted {
    fn from_vec(vec: Vec<String>) -> Self {
        Self(vec)
    }

    pub async fn get_by_id(
        user_id: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Self>, DynError> {
        match Self::get_from_index(user_id, skip, limit).await? {
            Some(mutes) => Ok(Some(Self::from_vec(mutes))),
            None => {
                let graph_response = Self::get_from_graph(user_id, skip, limit).await?;
                if let Some(follows) = graph_response {
                    follows.put_to_index(user_id).await?;
                    return Ok(Some(follows));
                }
                Ok(None)
            }
        }
    }

    async fn get_from_index(
        user_id: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Vec<String>>, DynError> {
        Self::try_from_index_set(&[user_id], skip, limit, None).await
    }

    async fn get_from_graph(
        user_id: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Self>, DynError> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query = queries::get::get_user_muted(user_id, skip, limit);

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        if let Some(row) = result.next().await? {
            let user_exists: bool = row.get("user_exists").unwrap_or(false);
            if !user_exists {
                return Ok(None);
            }

            match row.get::<Option<Vec<String>>>("muted_ids") {
                Ok(response) => {
                    if let Some(connections) = response {
                        Ok(Some(Self::from_vec(connections)))
                    } else {
                        Ok(Some(Self::default()))
                    }
                }
                Err(_e) => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    pub async fn put_to_index(&self, user_id: &str) -> Result<(), DynError> {
        let user_list_ref: Vec<&str> = self.as_ref().iter().map(|id| id.as_str()).collect();
        Self::put_index_set(&[user_id], &user_list_ref, None, None).await
    }

    pub async fn put_to_graph(user_id: &str, muted_id: &str) -> Result<OperationOutcome, DynError> {
        let indexed_at = Utc::now().timestamp_millis();
        let query = queries::put::create_mute(user_id, muted_id, indexed_at);
        execute_graph_operation(query).await
    }

    pub async fn reindex(user_id: &str) -> Result<(), DynError> {
        match Self::get_from_graph(user_id, None, None).await? {
            Some(muted) => muted.put_to_index(user_id).await?,
            None => tracing::error!(
                "{}: Could not found user muted relationship in the graph",
                user_id
            ),
        }
        Ok(())
    }

    pub async fn del_from_graph(
        user_id: &str,
        muted_id: &str,
    ) -> Result<OperationOutcome, DynError> {
        let query = queries::del::delete_mute(user_id, muted_id);
        execute_graph_operation(query).await
    }

    pub async fn del_from_index(&self, user_id: &str) -> Result<(), DynError> {
        self.remove_from_index_set(&[user_id]).await
    }

    // Checks whether a user is muted
    pub async fn check(user_id: &str, muted_id: &str) -> Result<bool, DynError> {
        let user_key_parts = &[user_id][..];
        let (_, muted) = Self::check_set_member(user_key_parts, muted_id).await?;
        Ok(muted)
    }
}
