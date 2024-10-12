use crate::models::post::{PostStream, PostStreamSorting, ViewerStreamSource};
use crate::routes::v0::endpoints::STREAM_POSTS_ROUTE;
use crate::routes::v0::queries::PostStreamQuery;
use crate::{Error, Result};
use axum::extract::Query;
use axum::Json;
use log::info;
use utoipa::OpenApi;

const MAX_TAGS: usize = 5;

#[utoipa::path(
    get,
    path = STREAM_POSTS_ROUTE,
    tag = "Stream Posts",
    params(
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("author_id" = Option<String>, Query, description = "Filter posts by an specific author User ID"),
        ("skip" = Option<usize>, Query, description = "Skip N posts"),
        ("limit" = Option<usize>, Query, description = "Retrieve N posts"),
        ("sorting" = Option<PostStreamSorting>, Query, description = "Sorting method"),
        ("source" = Option<ViewerStreamSource>, Query, description = "Source of posts for streams with viewer (following, followers, friends, bookmarks, all)"),
        ("tags" = Option<Vec<String>>, Query, description = "Filter by a list of comma separated tags (max 5). E.g.,`&tags=dev,free,opensource`. Only posts matching at least one of the tags will be returned.")
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
    let source = query.source.unwrap_or(ViewerStreamSource::All);

    if source != ViewerStreamSource::All && query.viewer_id.is_none() {
        return Err(Error::InvalidInput {
            message: "Viewer ID is required for streams with a source other than 'all'".to_string(),
        });
    }

    // Enforce maximum number of tags
    if let Some(ref tags) = query.tags {
        if tags.len() > MAX_TAGS {
            return Err(Error::InvalidInput {
                message: format!("Too many tags provided; maximum allowed is {}", MAX_TAGS),
            });
        }
    }

    match PostStream::get_posts(
        query.viewer_id,
        query.author_id,
        sorting,
        source,
        query.tags,
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
    components(schemas(PostStream, PostStreamSorting, ViewerStreamSource))
)]
pub struct StreamPostsApiDocs;
