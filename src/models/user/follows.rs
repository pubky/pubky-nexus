use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::{queries, RedisOps};
use axum::async_trait;
use neo4rs::Query;
use serde::{Deserialize, Serialize};
use std::error::Error;
use utoipa::ToSchema;

#[async_trait]
pub trait UserFollows: Sized + RedisOps + AsRef<[String]> + Default {
    fn from_vec(vec: Vec<String>) -> Self;

    async fn get_by_id(
        user_id: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
        match Self::try_from_index_set(&[user_id], skip, limit).await? {
            Some(connections) => Ok(Some(Self::from_vec(connections))),
            None => Self::get_from_graph(user_id, skip, limit).await,
        }
    }

    async fn get_from_graph(
        user_id: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
        let graph = get_neo4j_graph()?;
        let query = Self::get_query(user_id, skip, limit);

        let graph = graph.lock().await;
        let mut result = graph.execute(query).await?;

        if let Some(row) = result.next().await? {
            let user_exists: bool = row.get("user_exists").unwrap_or(false);
            if !user_exists {
                return Ok(None);
            }
            if let Some(connections) = row.get::<Option<Vec<String>>>(Self::get_ids_field_name())? {
                let connections = Self::from_vec(connections);
                connections.put_index_set(&[user_id]).await?;
                Ok(Some(connections))
            } else {
                Ok(Some(Self::default()))
            }
        } else {
            Ok(None)
        }
    }

    fn get_query(user_id: &str, skip: Option<usize>, limit: Option<usize>) -> Query;
    fn get_ids_field_name() -> &'static str;
}

#[derive(Serialize, Deserialize, ToSchema, Default)]
pub struct Followers(pub Vec<String>);

#[derive(Serialize, Deserialize, ToSchema, Default)]
pub struct Following(pub Vec<String>);

impl AsRef<[String]> for Followers {
    fn as_ref(&self) -> &[String] {
        &self.0
    }
}

impl AsRef<[String]> for Following {
    fn as_ref(&self) -> &[String] {
        &self.0
    }
}

#[async_trait]
impl RedisOps for Followers {}

#[async_trait]
impl RedisOps for Following {}

impl UserFollows for Followers {
    fn from_vec(vec: Vec<String>) -> Self {
        Self(vec)
    }

    fn get_query(user_id: &str, skip: Option<usize>, limit: Option<usize>) -> Query {
        queries::get_user_followers(user_id, skip, limit)
    }

    fn get_ids_field_name() -> &'static str {
        "follower_ids"
    }
}

impl UserFollows for Following {
    fn from_vec(vec: Vec<String>) -> Self {
        Self(vec)
    }

    fn get_query(user_id: &str, skip: Option<usize>, limit: Option<usize>) -> Query {
        queries::get_user_following(user_id, skip, limit)
    }

    fn get_ids_field_name() -> &'static str {
        "following_ids"
    }
}

#[derive(Serialize, Deserialize, ToSchema, Default)]
pub struct Friends(pub Vec<String>);

impl Friends {
    pub async fn get_by_id(
        user_id: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
        // Fetch following and followers, limit to 10K
        let following = Following::get_by_id(user_id, None, Some(10000))
            .await?
            .unwrap_or_default()
            .0;

        let followers = Followers::get_by_id(user_id, None, Some(10000))
            .await?
            .unwrap_or_default()
            .0;

        // Find intersection of following and followers (mutual friends)
        let mut friends: Vec<String> = following
            .into_iter()
            .filter(|user_id| followers.contains(user_id))
            .collect();

        if friends.is_empty() {
            return Ok(None);
        }

        if let Some(skip) = skip {
            friends = friends.into_iter().skip(skip).collect();
        }
        if let Some(limit) = limit {
            friends.truncate(limit);
        }

        Ok(Some(Self(friends)))
    }
}
