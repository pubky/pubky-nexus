use crate::models::user::Relationship;
use crate::routes::v0::endpoints::RELATIONSHIP_ROUTE;
use crate::{Error, Result};
use axum::extract::Path;
use axum::Json;
use log::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = RELATIONSHIP_ROUTE,
    tag = "User <> Viewer Relationship",
    params(
        ("user_id" = String, Path, description = "User Pubky ID"),
        ("viewer_id" = String, Path, description = "Viewer Pubky ID")
    ),
    responses(
        (status = 200, description = "User relationship", body = Relationship),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_relationship_handler(
    Path((user_id, viewer_id)): Path<(String, String)>,
) -> Result<Json<Relationship>> {
    info!(
        "GET {RELATIONSHIP_ROUTE} user_id:{}, viewer_id:{}",
        user_id, viewer_id
    );

    match Relationship::get_by_id(&user_id, Some(&viewer_id)).await {
        Ok(Some(relationship)) => Ok(Json(relationship)),
        Ok(None) => Err(Error::UserNotFound { user_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(paths(user_relationship_handler), components(schemas(Relationship)))]
pub struct RelationshipApiDoc;
