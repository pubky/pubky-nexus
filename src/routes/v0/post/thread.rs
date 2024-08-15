use crate::models::post::PostThread;
use crate::routes::v0::endpoints::THREAD_ROUTE;
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use log::info;
use serde::Deserialize;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct ThreadQuery {
    viewer_id: Option<String>,
    skip: Option<usize>,
    limit: Option<usize>,
}

#[utoipa::path(
    get,
    path = THREAD_ROUTE,
    tag = "Post Thread",
    params(
        ("author_id" = String, Path, description = "Author Pubky ID"),
        ("post_id" = String, Path, description = "Post ID"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("skip" = Option<usize>, Query, description = "Number of posts to skip for pagination"),
        ("limit" = Option<usize>, Query, description = "Number of posts to return for pagination")
    ),
    responses(
        (status = 200, description = "Post Thread", body = PostThread),
        (status = 404, description = "Post or thread not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn thread_handler(
    Path((author_id, post_id)): Path<(String, String)>,
    Query(query): Query<ThreadQuery>,
) -> Result<Json<PostThread>> {
    info!(
        "GET {THREAD_ROUTE} author_id:{}, post_id:{}, viewer_id:{:?}, skip:{:?}, limit:{:?}",
        author_id, post_id, query.viewer_id, query.skip, query.limit
    );

    let skip = query.skip.unwrap_or(0);
    let limit = query.limit.unwrap_or(6); // Default limit if not provided

    match PostThread::get_by_id(
        &author_id,
        &post_id,
        query.viewer_id.as_deref(),
        skip,
        limit,
    )
    .await
    {
        Ok(Some(thread)) => Ok(Json(thread)),
        Ok(None) => Err(Error::PostNotFound { author_id, post_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(paths(thread_handler), components(schemas(PostThread)))]
pub struct ThreadViewApiDoc;
