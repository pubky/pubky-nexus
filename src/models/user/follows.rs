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
            if let Some(connections) = row.get::<Option<Vec<String>>>(Self::get_ids_field_name())? {
                // TODO: DISCUSS, Might be one of the reasons why put_index_set was a method(with &self as 1st argument)
                // to avoid clonning
                let follows = Self::from_vec(connections);
                let values_ref: Vec<&str> = follows.as_ref().iter().map(|id| id.as_str()).collect();
                Self::put_index_set(&[user_id], &values_ref).await?;
                Ok(Some(follows))
            } else {
                Ok(Some(Self::default()))
            }
        } else {
            Ok(None)
        }
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

// TODO: Followers keep the track the people that the key is following. AKA it has you following. #followee_follower_inverse
#[derive(Serialize, Deserialize, ToSchema, Default)]
pub struct Followers(pub Vec<String>);

// TODO: Following keep the track the people that are following the key. AKA it has you followers. #followee_follower_inverse
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
        queries::read::get_user_followers(user_id, skip, limit)
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
        queries::read::get_user_following(user_id, skip, limit)
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

    // Checks wjether user_a and user_b are friends
    pub async fn check(
        user_a_id: &str,
        user_b_id: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let user_a_key_parts = &[user_a_id][..];
        let user_b_key_parts = &[user_b_id][..];

        let ((_, a_follows_b), (_, b_follows_a)) = tokio::try_join!(
            Following::check_set_member(user_a_key_parts, user_b_id),
            Following::check_set_member(user_b_key_parts, user_a_id),
        )?;

        Ok(a_follows_b && b_follows_a)
    }
}
