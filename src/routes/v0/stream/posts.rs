use crate::models::post::{PostStream, PostStreamSorting};
use crate::routes::v0::endpoints::STREAM_POSTS_ROUTE;
use crate::{Error, Result};
use axum::extract::Query;
use axum::Json;
use log::info;
use serde::Deserialize;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct PostStreamQuery {
    viewer_id: Option<String>,
    skip: Option<isize>,
    limit: Option<isize>,
    sorting: Option<PostStreamSorting>,
}

#[utoipa::path(
    get,
    path = STREAM_POSTS_ROUTE,
    tag = "Stream Posts",
    params(
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("skip" = Option<usize>, Query, description = "Skip N posts"),
        ("limit" = Option<usize>, Query, description = "Retrieve N posts"),
        ("sorting" = Option<PostStreamSorting>, Query, description = "Sorting method")
    ),
    responses(
        (status = 200, description = "Posts stream", body = PostStream),
        (status = 404, description = "Posts not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn stream_posts_handler(
    Query(query): Query<PostStreamQuery>,
) -> Result<Json<PostStream>> {
    info!("GET {STREAM_POSTS_ROUTE}");

    let skip = query.skip.unwrap_or(0);
    let limit = query.limit.unwrap_or(10);
    let sorting = query.sorting.unwrap_or(PostStreamSorting::Timeline);

    match PostStream::get_sorted_posts(sorting, query.viewer_id, Some(skip), Some(limit)).await {
        Ok(Some(stream)) => Ok(Json(stream)),
        Ok(None) => Err(Error::InternalServerError {
            source: "No posts found".into(),
        }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(stream_posts_handler),
    components(schemas(PostStream, PostStreamSorting))
)]
pub struct StreamPostsApiDocs;
