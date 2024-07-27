use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::{queries, RedisOps};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::error::Error;
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct Follows(Vec<String>);

#[derive(Debug, PartialEq)]
pub enum FollowsVariant {
    Followers,
    Following,
}

#[async_trait]
impl RedisOps for Follows {
    // async fn set_index(&self, key_parts: &[&str]) -> Result<(), Box<dyn Error + Send + Sync>> {
    //     let key = key_parts.join(":");
    //     crate::db::kv::index::set(&Self::prefix().await, &key, &self.0, None, None).await?;
    //     Ok(())
    // }

    // async fn try_from_index(
    //     key_parts: &[&str],
    // ) -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
    //     let key = key_parts.join(":");
    //     if let Some(user_ids) =
    //         crate::db::kv::index::get::<Vec<String>>(&Self::prefix().await, &key, None).await?
    //     {
    //         Ok(Some(Follows(user_ids)))
    //     } else {
    //         Ok(None)
    //     }
    // }
}
impl Default for Follows {
    fn default() -> Self {
        Self::new()
    }
}

impl Follows {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub async fn get_by_id(
        user_id: &str,
        variant: FollowsVariant,
    ) -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
        // let key = match variant {
        //     FollowsVariant::Followers => "followers",
        //     FollowsVariant::Following => "following",
        // };
        // match Self::try_from_index(&[user_id, key]).await? {
        //     Some(follows) => Ok(Some(follows)),
        //     None => Self::get_from_graph(user_id, relationship).await,
        // }
        Self::get_from_graph(user_id, variant).await
    }

    pub async fn get_from_graph(
        user_id: &str,
        variant: FollowsVariant,
    ) -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
        let graph = get_neo4j_graph()?;
        let query = match variant {
            FollowsVariant::Followers => queries::get_user_followers(user_id),
            FollowsVariant::Following => queries::get_user_following(user_id),
        };

        let mut follows = Vec::new();

        let graph = graph.lock().await;
        let mut result = graph.execute(query).await?;

        if let Some(row) = result.next().await? {
            let user_id_column = match variant {
                FollowsVariant::Followers => "follower_ids",
                FollowsVariant::Following => "following_ids",
            };

            if let Some(ids) = row.get::<Option<Vec<String>>>(user_id_column)? {
                follows.extend(ids);
            }
        }

        if !follows.is_empty() {
            // let follows = Follows(follows);
            // let key = match variant {
            //     FollowsVariant::Followers => "followers",
            //     FollowsVariant::Following => "following",
            // };
            // follows.set_index(&[user_id, key]).await?;
            Ok(Some(Self(follows)))
        } else {
            Ok(None)
        }
    }
}
