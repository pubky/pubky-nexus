use crate::routes::v0::endpoints::USER_COUNTS_ROUTE;
use crate::{Error, Result};
use axum::extract::Path;
use axum::Json;
use nexus_common::models::user::UserCounts;
use tracing::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = USER_COUNTS_ROUTE,
    tag = "User",
    description = "User counts",
    params(
        ("user_id" = String, Path, description = "User Pubky ID")
    ),
    responses(
        (status = 200, description = "User counts", body = UserCounts),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_counts_handler(Path(user_id): Path<String>) -> Result<Json<UserCounts>> {
    info!("GET {USER_COUNTS_ROUTE} user_id:{}", user_id);

    match UserCounts::get_by_id(&user_id).await {
        Ok(Some(counts)) => Ok(Json(counts)),
        Ok(None) => Err(Error::UserNotFound { user_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(paths(user_counts_handler), components(schemas(UserCounts)))]
pub struct UserCountsApiDoc;
