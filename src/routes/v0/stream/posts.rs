use crate::models::post::{PostStream, PostStreamReach, PostStreamSorting};
use crate::routes::v0::endpoints::STREAM_POSTS_ROUTE;
use crate::routes::v0::queries::PostStreamQuery;
use crate::{Error, Result};
use axum::extract::Query;
use axum::Json;
use log::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = STREAM_POSTS_ROUTE,
    tag = "Stream Posts",
    params(
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("author_id" = Option<String>, Query, description = "Author User ID to filter posts by a specific user"),
        ("skip" = Option<usize>, Query, description = "Skip N posts"),
        ("limit" = Option<usize>, Query, description = "Retrieve N posts"),
        ("sorting" = Option<PostStreamSorting>, Query, description = "Sorting method"),
        ("reach" = Option<PostStreamReach>, Query, description = "Reach type (following, followers, friends, bookmarks, all)"),
        ("tag" = Option<String>, Query, description = "Filter by tag label")
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
    let limit = query.limit.unwrap_or(10).min(30);
    let sorting = query.sorting.unwrap_or(PostStreamSorting::Timeline);
    let reach = query.reach.unwrap_or(PostStreamReach::All);

    if reach != PostStreamReach::All && query.viewer_id.is_none() {
        return Err(Error::InvalidInput {
            message: "Viewer ID is required for streams with a reach other than 'all'".to_string(),
        });
    }

    match PostStream::get_posts(
        query.viewer_id.clone(),
        query.author_id.clone(),
        sorting,
        reach,
        query.tag.clone(),
        Some(skip),
        Some(limit),
    )
    .await
    {
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
    components(schemas(PostStream, PostStreamSorting, PostStreamReach))
)]
pub struct StreamPostsApiDocs;
