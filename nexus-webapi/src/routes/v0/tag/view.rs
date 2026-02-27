use crate::routes::v0::endpoints::TAG_ROUTE;
use crate::{Error, Result};
use axum::extract::Path;
use axum::Json;
use nexus_common::models::tag::view::TagView;
use tracing::debug;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = TAG_ROUTE,
    description = "Tag view",
    tag = "Tag",
    params(
        ("tagger_id" = String, Path, description = "Tagger Pubky ID"),
        ("tag_id" = String, Path, description = "Tag Pubky ID"),
    ),
    responses(
        (status = 200, description = "Tag", body = TagView),
        (status = 404, description = "Tag not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn tag_view_handler(
    Path((tagger_id, tag_id)): Path<(String, String)>,
) -> Result<Json<TagView>> {
    debug!("GET {TAG_ROUTE} tagger_id:{}, tag_id:{}", tagger_id, tag_id);

    match TagView::get_by_tagger_and_id(&tagger_id, &tag_id).await? {
        Some(tag) => Ok(Json(tag)),
        None => Err(Error::TagNotFound { tag_id, tagger_id }),
    }
}

#[derive(OpenApi)]
#[openapi(paths(tag_view_handler), components(schemas(TagView)))]
pub struct TagViewApiDoc;
