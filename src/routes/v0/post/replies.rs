// use crate::models::post::PostThread;
// use crate::routes::v0::endpoints::POST_REPLIES_ROUTE;
// use crate::{Error, Result};
// use axum::extract::{Path, Query};
// use axum::Json;
// use log::info;
// use serde::Deserialize;
// use utoipa::OpenApi;

// #[derive(Deserialize)]
// pub struct PostRepliesQuery {
//     start: Option<usize>,
//     end: Option<usize>,
//     limit: Option<usize>,
// }

// #[utoipa::path(
//     get,
//     path = POST_REPLIES_ROUTE,
//     tag = "Post Replies",
//     params(
//         ("author_id" = String, Path, description = "Author Pubky ID"),
//         ("post_id" = String, Path, description = "Post ID"),
//         ("start" = Option<usize>, Query, description = "The timestamp that we want to start the replies timeframe"),
//         ("end" = Option<usize>, Query, description = "The timestamp that we want to end the replies timeframe"),
//         ("limit" = Option<usize>, Query, description = "Number of posts to return for pagination")
//     ),
//     responses(
//         (status = 200, description = "Post replies", body = PostRepliesQuery),
//         (status = 404, description = "Post replies not found"),
//         (status = 500, description = "Internal server error")
//     )
// )]
// pub async fn thread_handler(
//     Path((author_id, post_id)): Path<(String, String)>,
//     Query(query): Query<PostRepliesQuery>,
// ) -> Result<Json<Option<usize>>> {
//     info!(
//         "GET {POST_REPLIES_ROUTE} author_id:{}, post_id:{}, viewer_id:{:?}, skip:{:?}, limit:{:?}",
//         author_id, post_id, query.start, query.end, query.limit
//     );

//     let start = query.start.unwrap_or(0);
//     let end = query.end.unwrap_or(6).min(20); // Default limit if not provided
//     Ok(Json(None))
// }

// #[derive(OpenApi)]
// #[openapi(paths(thread_handler), components(schemas(PostThread)))]
// pub struct ThreadViewApiDoc;
