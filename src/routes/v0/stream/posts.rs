use crate::routes::v0::stream::queries::ViewerStreamSource;
use crate::routes::v0::stream::PostStreamSorting;
use crate::Error;
use axum::{extract::Query, Json};
use utoipa::OpenApi;

use crate::models::post::PostStream;
use crate::routes::v0::endpoints::STREAM_POSTS_ROUTE;

use super::queries::PostStreamQuery;

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
