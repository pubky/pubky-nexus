use crate::models::profile::Profile;
use axum::{
    extract::Path,
    response::{IntoResponse, Response},
    Json,
};
use pk_social_common::connectors::neo4j::Neo4jConnector;
use std::sync::Arc;

#[utoipa::path(
    get,
    path = "/v0/profiles/{userId}",
    params(
        ("userId" = String, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "User profile", body = Profile),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_profile(
    Path(user_id): Path<String>,
    neo4j_connector: Arc<Neo4jConnector>,
) -> Result<Json<Profile>, Response> {
    match neo4j_connector.get_user_by_id(&user_id).await {
        Ok(Some(node)) => {
            let profile = Profile::from_neo4j_user_node(&node);
            Ok(Json(profile))
        }
        Ok(None) => Err((axum::http::StatusCode::NOT_FOUND, "User not found").into_response()),
        Err(_) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error",
        )
            .into_response()),
    }
}
