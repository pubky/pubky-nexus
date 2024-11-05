use super::queries::PostStreamQuery;
use crate::models::post::{PostStream, PostStreamSorting, ViewerStreamSource};
use crate::routes::v0::endpoints::STREAM_POSTS_ROUTE;
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
        ("sorting" = Option<PostStreamSorting>, Query, description = "Sorting method"),
        ("author_id" = Option<String>, Query, description = "Filter posts by an specific author User ID"),
        ("source" = Option<ViewerStreamSource>, Query, description = "Source of posts for streams with viewer (following, followers, friends, bookmarks, replies, all)"),
        ("tags" = Option<Vec<String>>, Query, description = "Filter by a list of comma separated tags (max 5). E.g.,`&tags=dev,free,opensource`. Only posts matching at least one of the tags will be returned."),
        ("post_id" = Option<String>, Query, description = "This parameter is needed when we want to retrieve the replies stream for a post"),
        ("skip" = Option<usize>, Query, description = "Skip N posts"),
        ("limit" = Option<usize>, Query, description = "Retrieve N posts"),
        // TODO: Explain better start/end, sometimes the start could be a score, depending stream type. Do we need to have in that cases, start/end
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
) -> Result<Json<PostStream>> {
    info!("GET {STREAM_POSTS_ROUTE}");

    query.initialize_defaults();

    let source_ref = query.filters.source.as_ref().unwrap();

    if !viewer_param_optional(source_ref) && query.viewer_id.is_none() {
        return Err(Error::InvalidInput {
            message:
                "Viewer ID is required for streams with a source other than 'all' or 'replies'"
                    .to_string(),
        });
    }

    // Enforce maximum number of tags
    if let Some(ref tags) = query.filters.tags {
        if tags.len() > MAX_TAGS {
            return Err(Error::InvalidInput {
                message: format!("Too many tags provided; maximum allowed is {}", MAX_TAGS),
            });
        }
    }

    if source_ref == &ViewerStreamSource::Replies
        && (query.filters.post_id.is_none() || query.filters.author_id.is_none())
    {
        return Err(Error::InvalidInput {
            message: "Post ID is required for streams with a source 'Replies'".to_string(),
        });
    }

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
    components(schemas(PostStream, PostStreamSorting, ViewerStreamSource))
)]
pub struct StreamPostsApiDocs;

fn viewer_param_optional(source: &ViewerStreamSource) -> bool {
    matches!(
        source,
        ViewerStreamSource::All | ViewerStreamSource::Replies
    )
}
