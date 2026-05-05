use crate::models::{PostId, PubkyId};
use crate::routes::v0::endpoints::POST_DETAILS_ROUTE;
use crate::routes::v0::post::view::PostPath;
use crate::routes::Path;
use crate::{Error, Result};
use axum::Json;
use nexus_common::models::post::PostDetails;
use pubky_app_specs::PubkyAppPostKind;
use tracing::debug;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = POST_DETAILS_ROUTE,
    description = "Post details",
    tag = "Post",
    params(
        ("author_id" = PubkyId, Path, description = "Author Pubky ID"),
        ("post_id" = PostId, Path, description = "Post Crockford32 ID")
    ),
    responses(
        (status = 200, description = "Post Details", body = PostDetails),
        (status = 404, description = "Post not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn post_details_handler(
    Path(PostPath { author_id, post_id }): Path<PostPath>,
) -> Result<Json<PostDetails>> {
    debug!(
        "GET {POST_DETAILS_ROUTE} author_id:{}, post_id:{}",
        author_id, post_id
    );

    match PostDetails::get_by_id(&author_id, &post_id).await? {
        Some(post) => Ok(Json(post)),
        None => Err(Error::PostNotFound { author_id, post_id }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(post_details_handler),
    components(schemas(PostDetails, PubkyAppPostKind, PubkyId, PostId))
)]
pub struct PostDetailsApiDoc;
