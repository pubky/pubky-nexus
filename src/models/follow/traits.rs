use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::db::graph::exec::exec_boolean_row;
use crate::{queries, RedisOps};
use axum::async_trait;
use chrono::Utc;
use neo4rs::Query;
use std::error::Error;

#[async_trait]
pub trait UserFollows: Sized + RedisOps + AsRef<[String]> + Default {
    fn from_vec(vec: Vec<String>) -> Self;

    async fn put_to_graph(
        follower_id: &str,
        followee_id: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let indexed_at = Utc::now().timestamp_millis();
        let query = queries::put::create_follow(follower_id, followee_id, indexed_at);
        exec_boolean_row(query).await
    }

    async fn get_by_id(
        user_id: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
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
    ) -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query = Self::get_query(user_id, skip, limit);

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        if let Some(row) = result.next().await? {
            let user_exists: bool = row.get("user_exists").unwrap_or(false);
            if !user_exists {
                return Ok(None);
            }

            match row.get::<Option<Vec<String>>>(Self::get_ids_field_name()) {
                Ok(response) => {
                    if let Some(connections) = response {
                        return Ok(Some(Self::from_vec(connections)));
                    } else {
                        return Ok(Some(Self::default()));
                    }
                }
                Err(_e) => return Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    async fn get_from_index(
        user_id: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Vec<String>>, Box<dyn Error + Send + Sync>> {
        Self::try_from_index_set(&[user_id], skip, limit).await
    }

    async fn put_to_index(
        &self,
        user_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let user_list_ref: Vec<&str> = self.as_ref().iter().map(|id| id.as_str()).collect();
        Self::put_index_set(&[user_id], &user_list_ref).await
    }

    async fn reindex(user_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match Self::get_from_graph(user_id, None, None).await? {
            Some(follow) => follow.put_to_index(user_id).await?,
            None => log::error!(
                "{}: Could not found user follow relationship in the graph",
                user_id
            ),
        }
        Ok(())
    }

    async fn del_from_graph(
        follower_id: &str,
        followee_id: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let query = queries::del::delete_follow(follower_id, followee_id);
        exec_boolean_row(query).await
    }

    async fn del_from_index(
        &self,
        user_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.remove_from_index_set(&[user_id]).await
    }

    fn get_query(user_id: &str, skip: Option<usize>, limit: Option<usize>) -> Query;

    fn get_ids_field_name() -> &'static str;

    // Checks whether user_a is (following | follower) of user_b
    async fn check(user_a_id: &str, user_b_id: &str) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let user_a_key_parts = &[user_a_id][..];
        let (_, follow) = Self::check_set_member(user_a_key_parts, user_b_id).await?;
        Ok(follow)
    }
}
