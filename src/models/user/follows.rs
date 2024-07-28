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
        match Self::try_from_follows_indexes(user_id, &variant, None, None).await? {
            Some(follows) => Ok(Some(Self(follows))),
            None => Self::get_from_graph(user_id, &variant).await,
        }
    }

    async fn try_from_follows_indexes(
        user_id: &str,
        variant: &FollowsVariant,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Vec<String>>, Box<dyn Error + Send + Sync>> {
        let (pattern, position) = match variant {
            FollowsVariant::Followers => (format!("*:{user_id}"), 1),
            FollowsVariant::Following => (format!("*:{user_id}:*"), 2),
        };
        let (keys, _) = index::get_bool_range(
            &Self::prefix().await,
            Some(pattern.as_str()),
            skip,
            limit,
            index::RangeReturnType::Keys,
        )
        .await?;

        if let Some(keys) = keys {
            if !keys.is_empty() {
                let ids = keys
                    .into_iter()
                    .map(|k| {
                        k.split(':')
                            .nth(position) // Extracts the relevant user ID part from the key
                            .unwrap()
                            .to_string()
                    })
                    .collect();
                Ok(Some(ids))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    pub async fn get_from_graph(
        user_id: &str,
        variant: &FollowsVariant,
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
