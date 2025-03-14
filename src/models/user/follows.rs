// use crate::db::connectors::neo4j::get_neo4j_graph;
// use crate::db::graph::exec::exec_boolean_row;
// use crate::{queries, RedisOps};
// use async_trait::async_trait;
// use chrono::Utc;
// use neo4rs::Query;
// use serde::{Deserialize, Serialize};
// use std::error::Error;
// use utoipa::ToSchema;
// use crate::types::DynError;

// #[async_trait]
// pub trait UserFollows: Sized + RedisOps + AsRef<[String]> + Default {
//     fn from_vec(vec: Vec<String>) -> Self;

//     async fn put_to_graph(
//         follower_id: &str,
//         followee_id: &str,
//     ) -> Result<bool, DynError> {
//         let indexed_at = Utc::now().timestamp_millis();
//         let query = queries::put::create_follow(follower_id, followee_id, indexed_at);
//         exec_boolean_row(query).await
//     }

//     async fn get_by_id(
//         user_id: &str,
//         skip: Option<usize>,
//         limit: Option<usize>,
//     ) -> Result<Option<Self>, DynError> {
//         match Self::get_from_index(user_id, skip, limit).await? {
//             Some(connections) => Ok(Some(Self::from_vec(connections))),
//             None => {
//                 let graph_response = Self::get_from_graph(user_id, skip, limit).await?;
//                 if let Some(follows) = graph_response {
//                     follows.put_to_index(user_id).await?;
//                     return Ok(Some(follows));
//                 }
//                 Ok(None)
//             }
//         }
//     }

//     async fn get_from_graph(
//         user_id: &str,
//         skip: Option<usize>,
//         limit: Option<usize>,
//     ) -> Result<Option<Self>, DynError> {
//         let mut result;
//         {
//             let graph = get_neo4j_graph()?;
//             let query = Self::get_query(user_id, skip, limit);

//             let graph = graph.lock().await;
//             result = graph.execute(query).await?;
//         }

//         if let Some(row) = result.next().await? {
//             let user_exists: bool = row.get("user_exists").unwrap_or(false);
//             if !user_exists {
//                 return Ok(None);
//             }

//             match row.get::<Option<Vec<String>>>(Self::get_ids_field_name()) {
//                 Ok(response) => {
//                     if let Some(connections) = response {
//                         return Ok(Some(Self::from_vec(connections)));
//                     } else {
//                         return Ok(Some(Self::default()));
//                     }
//                 }
//                 Err(_e) => return Ok(None),
//             }
//         } else {
//             Ok(None)
//         }
//     }

//     async fn get_from_index(
//         user_id: &str,
//         skip: Option<usize>,
//         limit: Option<usize>,
//     ) -> Result<Option<Vec<String>>, DynError> {
//         Self::try_from_index_set(&[user_id], skip, limit).await
//     }

//     async fn put_to_index(
//         &self,
//         user_id: &str,
//     ) -> Result<(), DynError> {
//         let user_list_ref: Vec<&str> = self.as_ref().iter().map(|id| id.as_str()).collect();
//         Self::put_index_set(&[user_id], &user_list_ref).await
//     }

//     async fn reindex(user_id: &str) -> Result<(), DynError> {
//         match Self::get_from_graph(user_id, None, None).await? {
//             Some(follow) => follow.put_to_index(user_id).await?,
//             None => tracing::error!(
//                 "{}: Could not found user follow relationship in the graph",
//                 user_id
//             ),
//         }
//         Ok(())
//     }

//     async fn del_from_graph(
//         follower_id: &str,
//         followee_id: &str,
//     ) -> Result<bool, DynError> {
//         let query = queries::del::delete_follow(follower_id, followee_id);
//         exec_boolean_row(query).await
//     }

//     async fn del_from_index(
//         &self,
//         user_id: &str,
//     ) -> Result<(), DynError> {
//         self.remove_from_index_set(&[user_id]).await
//     }

//     fn get_query(user_id: &str, skip: Option<usize>, limit: Option<usize>) -> Query;

//     fn get_ids_field_name() -> &'static str;

//     // Checks whether user_a is (following | follower) of user_b
//     async fn check(user_a_id: &str, user_b_id: &str) -> Result<bool, DynError> {
//         let user_a_key_parts = &[user_a_id][..];
//         let (_, follow) = Self::check_set_member(user_a_key_parts, user_b_id).await?;
//         Ok(follow)
//     }
// }

// #[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
// pub struct Followers(pub Vec<String>);

// // #[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
// // pub struct Following(pub Vec<String>);

// impl AsRef<[String]> for Followers {
//     fn as_ref(&self) -> &[String] {
//         &self.0
//     }
// }

// impl AsRef<[String]> for Following {
//     fn as_ref(&self) -> &[String] {
//         &self.0
//     }
// }

// #[async_trait]
// impl RedisOps for Followers {}

// // #[async_trait]
// // impl RedisOps for Following {}

// impl UserFollows for Followers {
//     fn from_vec(vec: Vec<String>) -> Self {
//         Self(vec)
//     }

//     fn get_query(user_id: &str, skip: Option<usize>, limit: Option<usize>) -> Query {
//         queries::get::get_user_followers(user_id, skip, limit)
//     }

//     fn get_ids_field_name() -> &'static str {
//         "follower_ids"
//     }
// }

// impl UserFollows for Following {
//     fn from_vec(vec: Vec<String>) -> Self {
//         Self(vec)
//     }

//     fn get_query(user_id: &str, skip: Option<usize>, limit: Option<usize>) -> Query {
//         queries::get::get_user_following(user_id, skip, limit)
//     }

//     fn get_ids_field_name() -> &'static str {
//         "following_ids"
//     }
// }

// #[derive(Serialize, Deserialize, ToSchema, Default)]
// pub struct Friends(pub Vec<String>);

// impl Friends {
//     pub async fn get_by_id(
//         user_id: &str,
//         skip: Option<usize>,
//         limit: Option<usize>,
//     ) -> Result<Option<Self>, DynError> {
//         // Fetch following and followers, limit to 10K
//         let following = Following::get_by_id(user_id, None, Some(10000))
//             .await?
//             .unwrap_or_default()
//             .0;

//         let followers = Followers::get_by_id(user_id, None, Some(10000))
//             .await?
//             .unwrap_or_default()
//             .0;

//         // Find intersection of following and followers (mutual friends)
//         let mut friends: Vec<String> = following
//             .into_iter()
//             .filter(|user_id| followers.contains(user_id))
//             .collect();

//         if friends.is_empty() {
//             return Ok(None);
//         }

//         if let Some(skip) = skip {
//             friends = friends.into_iter().skip(skip).collect();
//         }
//         if let Some(limit) = limit {
//             friends.truncate(limit);
//         }

//         Ok(Some(Self(friends)))
//     }

//     // Checks wjether user_a and user_b are friends
//     pub async fn check(
//         user_a_id: &str,
//         user_b_id: &str,
//     ) -> Result<bool, DynError> {
//         let user_a_key_parts = &[user_a_id][..];
//         let user_b_key_parts = &[user_b_id][..];

//         let ((_, a_follows_b), (_, b_follows_a)) = tokio::try_join!(
//             Following::check_set_member(user_a_key_parts, user_b_id),
//             Following::check_set_member(user_b_key_parts, user_a_id),
//         )?;

//         Ok(a_follows_b && b_follows_a)
//     }
// }
