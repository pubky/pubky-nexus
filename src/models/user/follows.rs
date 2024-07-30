use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::{queries, RedisOps};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::error::Error;
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct Followers(pub Vec<String>);

impl AsRef<[String]> for Followers {
    fn as_ref(&self) -> &[String] {
        &self.0
    }
}

#[async_trait]
impl RedisOps for Followers {}

impl Default for Followers {
    fn default() -> Self {
        Self::new()
    }
}

impl Followers {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub async fn get_by_id(
        user_id: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
        match Self::try_from_index_set(&[user_id], skip, limit).await? {
            Some(followers) => Ok(Some(Self(followers))),
            None => Self::get_from_graph(user_id, skip, limit).await,
        }
    }

    pub async fn get_from_graph(
        user_id: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
        let graph = get_neo4j_graph()?;
        let query = queries::get_user_followers(user_id, skip, limit);

        let graph = graph.lock().await;
        let mut result = graph.execute(query).await?;

        if let Some(row) = result.next().await? {
            let user_exists: bool = row.get("user_exists").unwrap_or(false);
            if !user_exists {
                return Ok(None);
            }
            if let Some(followers) = row.get::<Option<Vec<String>>>("follower_ids")? {
                let followers = Self(followers);
                followers.set_index_set(&[user_id]).await?;
                Ok(Some(followers))
            } else {
                Ok(Some(Followers::new()))
            }
        } else {
            Ok(None)
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct Following(pub Vec<String>);

impl AsRef<[String]> for Following {
    fn as_ref(&self) -> &[String] {
        &self.0
    }
}

#[async_trait]
impl RedisOps for Following {}

impl Default for Following {
    fn default() -> Self {
        Self::new()
    }
}

impl Following {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub async fn get_by_id(
        user_id: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
        match Self::try_from_index_set(&[user_id], skip, limit).await? {
            Some(following) => Ok(Some(Self(following))),
            None => Self::get_from_graph(user_id, skip, limit).await,
        }
    }

    pub async fn get_from_graph(
        user_id: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
        let graph = get_neo4j_graph()?;
        let query = queries::get_user_following(user_id, skip, limit);

        let graph = graph.lock().await;
        let mut result = graph.execute(query).await?;

        if let Some(row) = result.next().await? {
            let user_exists: bool = row.get("user_exists").unwrap_or(false);
            if !user_exists {
                return Ok(None);
            }
            if let Some(following) = row.get::<Option<Vec<String>>>("following_ids")? {
                let following = Self(following);
                following.set_index_set(&[user_id]).await?;
                Ok(Some(following))
            } else {
                Ok(Some(Following::new()))
            }
        } else {
            Ok(None)
        }
    }
}
