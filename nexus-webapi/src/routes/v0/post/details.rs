use crate::models::{PostId, PubkyId};
use crate::routes::v0::endpoints::POST_DETAILS_ROUTE;
use crate::routes::v0::post::view::PostPath;
use crate::routes::v0::version::{versioned_handlers, ApiVersion};
use crate::routes::Path;
use crate::{Error, Result};
use axum::Json;
use nexus_common::models::post::PostDetails;
use tracing::debug;
use utoipa::OpenApi;

/// The one body behind `post_details_handler_v0` and `post_details_handler_v1`: the details
/// in the version's shape.
pub async fn post_details<V: ApiVersion>(
    Path(PostPath { author_id, post_id }): Path<PostPath>,
) -> Result<Json<V::PostDetails>> {
    debug!(
        "GET {POST_DETAILS_ROUTE} author_id:{}, post_id:{}",
        author_id, post_id
    );

    match PostDetails::get_by_id(&author_id, &post_id).await? {
        Some(post) => Ok(Json(V::PostDetails::from(post))),
        None => Err(Error::post_not_found(author_id, post_id)),
    }
}

versioned_handlers! {
    post_details_handler_v0 / post_details_handler_v1 => post_details(path: Path<PostPath>) -> shapes::PostDetails;
    #[utoipa::path(
        get,
        path = POST_DETAILS_ROUTE,
        operation_id = "post_details_handler",
        description = "Post details",
        tag = "Post",
        params(
            ("author_id" = PubkyId, Path, description = "Author Pubky ID"),
            ("post_id" = PostId, Path, description = "Post Crockford32 ID")
        )
    )]
    responses(
        (status = 200, description = "Post Details"),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "Post not found"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
}

/// The schemas the details nest in every version; the details themselves are each version's.
#[derive(OpenApi)]
#[openapi(components(schemas(PubkyId, PostId)))]
pub struct PostDetailsApiDoc;
