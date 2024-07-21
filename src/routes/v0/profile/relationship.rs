use axum::extract::Path;
use axum::routing::get;
use axum::{Json, Router};
use log::info;
use utoipa::OpenApi;

use crate::models::profile::Relationship;
use crate::routes::v0::endpoints::RELATIONSHIP_ROUTE;
use crate::{Error, Result};

#[utoipa::path(
    get,
    path = RELATIONSHIP_ROUTE,
    tag = "Profile Viewer Relationship",
    params(
        ("user_id" = String, Path, description = "User ID"),
        ("viewer_id" = String, Path, description = "Viewer ID")
    ),
    responses(
        (status = 200, description = "User relationship", body = Relationship),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn profile_relationship_handler(
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

pub fn route() -> Router {
    Router::new().route(RELATIONSHIP_ROUTE, get(profile_relationship_handler))
}

#[derive(OpenApi)]
#[openapi(paths(profile_relationship_handler), components(schemas(Relationship)))]
pub struct RelationshipApiDoc;
