use crate::db::{
    execute_graph_operation, fetch_row_from_graph, queries, OperationOutcome, RedisOps,
};
use crate::types::DynError;
use async_trait::async_trait;
use chrono::Utc;
use neo4rs::Query;

#[async_trait]
pub trait UserFollows: Sized + RedisOps + AsRef<[String]> + Default {
    fn from_vec(vec: Vec<String>) -> Self;

    async fn put_to_graph(
        follower_id: &str,
        followee_id: &str,
    ) -> Result<OperationOutcome, DynError> {
        let indexed_at = Utc::now().timestamp_millis();
        let query = queries::put::create_follow(follower_id, followee_id, indexed_at);
        execute_graph_operation(query).await
    }

    async fn get_by_id(
        user_id: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Self>, DynError> {
        match Self::get_from_index(user_id, skip, limit).await? {
            Some(connections) => Ok(Some(Self::from_vec(connections))),
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

    async fn get_from_graph(
        user_id: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Self>, DynError> {
        let query = Self::get_query(user_id, skip, limit);
        let maybe_row = fetch_row_from_graph(query).await?;

        let Some(row) = maybe_row else {
            return Ok(None);
        };

        let user_exists: bool = row.get("user_exists").unwrap_or(false);
        if !user_exists {
            return Ok(None);
        }

        let user_follows = match row.get(Self::get_ids_field_name()) {
            Ok(Some(connections)) => Some(Self::from_vec(connections)),
            Ok(None) => Some(Self::default()),
            Err(_e) => None,
        };

        Ok(user_follows)
    }

    async fn get_from_index(
        user_id: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Vec<String>>, DynError> {
        Self::try_from_index_set(&[user_id], skip, limit, None)
            .await
            .map_err(Into::into)
    }

    async fn put_to_index(&self, user_id: &str) -> Result<(), DynError> {
        let user_list_ref: Vec<&str> = self.as_ref().iter().map(|id| id.as_str()).collect();
        Self::put_index_set(&[user_id], &user_list_ref, None, None)
            .await
            .map_err(Into::into)
    }

    async fn reindex(user_id: &str) -> Result<(), DynError> {
        match Self::get_from_graph(user_id, None, None).await? {
            Some(follow) => follow.put_to_index(user_id).await?,
            None => tracing::error!(
                "{}: Could not found user follow relationship in the graph",
                user_id
            ),
        }
        Ok(())
    }

    async fn del_from_graph(
        follower_id: &str,
        followee_id: &str,
    ) -> Result<OperationOutcome, DynError> {
        let query = queries::del::delete_follow(follower_id, followee_id);
        execute_graph_operation(query).await
    }

    async fn del_from_index(&self, user_id: &str) -> Result<(), DynError> {
        self.remove_from_index_set(&[user_id])
            .await
            .map_err(Into::into)
    }

    fn get_query(user_id: &str, skip: Option<usize>, limit: Option<usize>) -> Query;

    fn get_ids_field_name() -> &'static str;

    // Checks whether user_a is (following | follower) of user_b
    async fn check(user_a_id: &str, user_b_id: &str) -> Result<bool, DynError> {
        let user_a_key_parts = &[user_a_id][..];
        let (_, follow) = Self::check_set_member(user_a_key_parts, user_b_id).await?;
        Ok(follow)
    }
}
