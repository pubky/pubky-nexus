use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::db::kv::index;
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

impl AsRef<[String]> for Follows {
    fn as_ref(&self) -> &[String] {
        &self.0
    }
}

#[async_trait]
impl RedisOps for Follows {
    async fn set_multiple_indexes<T>(
        &self,
        key_parts_list: &[&[&str]],
    ) -> Result<(), Box<dyn Error + Send + Sync>>
    where
        Self: AsRef<[T]>,
        T: Serialize + Send + Sync,
    {
        let data: Vec<_> = key_parts_list
            .iter()
            .map(|key_parts| (key_parts.join(":"), true))
            .collect();

        index::set_multiple::<bool>(&Self::prefix().await, &data).await?;
        Ok(())
    }

    // async fn try_from_index(
    //     key_parts: &[&str],
    // ) -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
    //     let key = key_parts.join(":");
    //     let (keys, _values) = crate::db::kv::index::get_bool_range(
    //         &Self::prefix().await,
    //         Some(&format!(":{}:*", key_parts[1])),
    //         None,
    //         None,
    //         RangeReturnType::Keys,
    //     )
    //     .await?;

    //     if keys.is_empty() {
    //         Ok(None)
    //     } else {
    //         let ids = keys
    //             .into_iter()
    //             .map(|k| k.split(':').last().unwrap().to_string())
    //             .collect();
    //         Ok(Some(Follows(ids)))
    //     }
    // }
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
            let follows = Self(follows);
            let key_parts_list: Vec<Vec<&str>> = follows
                .0
                .iter()
                .map(|id| match variant {
                    FollowsVariant::Followers => vec![id.as_str(), user_id], // Follows:{follower_id}:{user_id}
                    FollowsVariant::Following => vec![user_id, id.as_str()], // Follows:{user_id}:{following_id}
                })
                .collect();

            let key_parts_slices: Vec<&[&str]> =
                key_parts_list.iter().map(|v| v.as_slice()).collect();

            follows.set_multiple_indexes(&key_parts_slices).await?;
            Ok(Some(follows))
        } else {
            Ok(None)
        }
    }
}
