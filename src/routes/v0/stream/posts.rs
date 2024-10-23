use crate::models::post::{PostStream, PostStreamSorting, ViewerStreamSource};
use crate::routes::v0::endpoints::STREAM_POSTS_ROUTE;
use crate::routes::v0::queries::PostStreamQuery;
use crate::{Error, Result};
use axum::extract::Query;
use axum::Json;
use log::info;
use utoipa::OpenApi;

use super::utils::{PostStreamFilters, PostStreamValues};

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
        ("source" = Option<ViewerStreamSource>, Query, description = "Source of posts for streams with viewer (following, followers, friends, bookmarks, replies, all)"),
        ("tags" = Option<Vec<String>>, Query, description = "Filter by a list of comma separated tags (max 5). E.g.,`&tags=dev,free,opensource`. Only posts matching at least one of the tags will be returned."),
        ("post_id" = Option<String>, Query, description = "This parameter is needed when we want to retrieve the replies stream for a post"),
        ("start" = Option<usize>, Query, description = "The start of the stream timeframe. Posts with a timestamp greater than this value will be excluded from the results"),
        ("end" = Option<usize>, Query, description = "The end of the stream timeframe. Posts with a timestamp less than this value will be excluded from the results"),
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

    if !viewer_query_optional(&source) && query.viewer_id.is_none() {
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

    if source == ViewerStreamSource::Replies
        && (query.post_id.is_none() || query.author_id.is_none())
    {
        return Err(Error::InvalidInput {
            message: "Post ID is required for streams with a source 'Replies'".to_string(),
        });
    }

    let post_stream_values =
        PostStreamValues::new(query.viewer_id, query.author_id, query.tags, query.post_id);

    let post_stream_filters = PostStreamFilters::new(
        sorting,
        source,
        Some(skip),
        Some(limit),
        query.start,
        query.end,
    );

    match PostStream::get_posts(post_stream_values, post_stream_filters).await {
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

fn viewer_query_optional(source: &ViewerStreamSource) -> bool {
    matches!(
        source,
        ViewerStreamSource::All | ViewerStreamSource::Replies
    )
}
