use axum::http::StatusCode;
use axum::{extract::Query, response::IntoResponse, Json};
use log::info;
use thiserror::Error;
use utoipa::OpenApi;
use crate::{Error, Result};

use crate::models::post::{PostStream, PostStreamSorting, ViewerStreamSource};
use crate::routes::v0::endpoints::STREAM_POSTS_ROUTE;

use super::queries::PostStreamQuery;

// #[derive(Debug, Error)]
// pub enum Error {
//     #[error("Missing parameter: {0}")]
//     MissingParam(String),
//     #[error("Invalid source: {0}")]
//     InvalidSource(String),
//     #[error("Invalid query: {0}")]
//     InvalidQuery(String),
//     #[error("Empty stream: {message}")]
//     EmptyStream { message: String },
//     #[error("Internal server error")]
//     InternalServerError,
// }

// impl IntoResponse for Error {
//     fn into_response(self) -> axum::response::Response {
//         let (status, body) = match self {
//             Error::MissingParam(msg) => (
//                 StatusCode::BAD_REQUEST,
//                 format!("Missing parameter: {}", msg),
//             ),
//             Error::InvalidSource(msg) => {
//                 (StatusCode::BAD_REQUEST, format!("Invalid source: {}", msg))
//             }
//             Error::InvalidQuery(msg) => {
//                 (StatusCode::BAD_REQUEST, format!("Invalid query: {}", msg))
//             }
//             Error::EmptyStream { message } => (StatusCode::NOT_FOUND, message),
//             _ => (
//                 StatusCode::INTERNAL_SERVER_ERROR,
//                 "Internal server error".to_string(),
//             ),
//         };
//         (status, body).into_response()
//     }
// }

type AppResult<T> = std::result::Result<T, Error>;

const MAX_TAGS: usize = 5;

#[utoipa::path(
    get,
    path = STREAM_POSTS_ROUTE,
    tag = "Stream Posts",
    params(
        ("source" = Option<ViewerStreamSource>, Query, description = "Source of posts for streams with viewer (following, followers, friends, bookmarks, replies, all)"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("observer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("author_id" = Option<String>, Query, description = "Filter posts by an specific author User ID"),
        ("post_id" = Option<String>, Query, description = "This parameter is needed when we want to retrieve the replies stream for a post"),
        ("sorting" = Option<PostStreamSorting>, Query, description = "Sorting method"),
        ("tags" = Option<Vec<String>>, Query, description = "Filter by a list of comma-separated tags (max 5). E.g.,`&tags=dev,free,opensource`. Only posts matching at least one of the tags will be returned."),
        ("skip" = Option<usize>, Query, description = "Skip N posts"),
        ("limit" = Option<usize>, Query, description = "Retrieve N posts"),
        ("start" = Option<usize>, Query, description = "The start of the stream timeframe or score. Posts with a timestamp/score greater than this value will be excluded from the results"),
        ("end" = Option<usize>, Query, description = "The end of the stream timeframe or score. Posts with a timestamp/score less than this value will be excluded from the results"),
    ),
    responses(
        (status = 200, description = "Posts stream", body = PostStream),
        (status = 404, description = "Posts not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn stream_posts_handler(
    Query(mut query): Query<PostStreamQuery>,
) -> AppResult<Json<PostStream>> {
    query.initialize_defaults();
    // Enforce maximum number of tags
    if let Some(ref tags) = query.filters.tags {
        if tags.len() > MAX_TAGS {
            return Err(Error::InvalidInput {
                message: format!("Too many tags provided; maximum allowed is {}", MAX_TAGS),
            });
        }
    }
    println!("QUERY: {:?}", query);

    // Use `query.source` as needed
    println!("Parsed source: {:?}", query.source);

    // ... rest of your handler logic

    match PostStream::get_posts(query).await {
        Ok(Some(stream)) => Ok(Json(stream)),
        Ok(None) => Err(Error::EmptyStream {
            message: "No posts found for the given criteria.".to_string(),
        }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(stream_posts_handler,),
    components(schemas(PostStream, PostStreamSorting))
)]
pub struct StreamPostsApiDocs;
