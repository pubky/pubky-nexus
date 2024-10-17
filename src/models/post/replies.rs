// use super::{PostStream, PostView};
// use crate::db::connectors::neo4j::get_neo4j_graph;
// use crate::db::graph::exec::exec_existed_row;
// use crate::models::user::PubkyId;
// use crate::{queries, RedisOps};
// use chrono::Utc;
// use serde::{Deserialize, Serialize};
// use utoipa::ToSchema;

// /// Represents post data with content, bio, image, links, and status.
// #[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
// // NOTE: Might not be necessary the default values for serde because before PUT a PostDetails node
// // we do sanity check
// pub struct PostReplies(pub Vec<PostView>);

// impl RedisOps for PostReplies {}

// impl PostReplies {
//     /// Retrieves post details by author ID and post ID, first trying to get from Redis, then from Neo4j if not found.
//     pub async fn get_by_id(
//         author_id: &str,
//         post_id: &str,
//     ) -> Result<Option<PostReplies>, Box<dyn std::error::Error + Send + Sync>> {
//         match Self::get_from_index(author_id, post_id).await? {
//             Some(details) => Ok(Some(details)),
//             None => {
//                 let graph_response = Self::get_from_graph(author_id, post_id).await?;
//                 if let Some((post_details, is_reply)) = graph_response {
//                     post_details.put_to_index(author_id, !is_reply).await?;
//                     return Ok(Some(post_details));
//                 }
//                 Ok(None)
//             }
//         }
//     }

//     pub async fn get_from_index(
//         author_id: &str,
//         post_id: &str,
//     ) -> Result<Option<PostReplies>, Box<dyn std::error::Error + Send + Sync>> {
//         if let Some(post_details) = Self::try_from_index_json(&[author_id, post_id]).await? {
//             return Ok(Some(post_details));
//         }
//         Ok(None)
//     }

//     /// Retrieves the post fields from Neo4j.
//     pub async fn get_from_graph(
//         author_id: &str,
//         post_id: &str,
//     ) -> Result<Option<(PostReplies, bool)>, Box<dyn std::error::Error + Send + Sync>> {
//         let mut result;
//         {
//             let graph = get_neo4j_graph()?;
//             let query = queries::get::get_post_by_id(author_id, post_id);

//             let graph = graph.lock().await;
//             result = graph.execute(query).await?;
//         }

//         match result.next().await? {
//             Some(row) => {
//                 let post: PostReplies = row.get("details")?;
//                 let is_reply: bool = row.get("is_reply").unwrap_or(false);
//                 Ok(Some((post, is_reply)))
//             }
//             None => Ok(None),
//         }
//     }

//     pub async fn put_to_index(
//         &self,
//         author_id: &str,
//         add_to_feeds: bool,
//     ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//         Ok(())
//     }

//     // Save new graph node
//     pub async fn put_to_graph(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
//        Ok(true)
//     }
// }
