use crate::routes::v0::endpoints::USER_MUTED_ROUTE;
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use nexus_common::models::user::Muted;
use nexus_common::types::Pagination;
use tracing::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = USER_MUTED_ROUTE,
    description = "List user's muted IDs",
    tag = "User",
    params(
        ("user_id" = String, Path, description = "User Pubky ID"),
        ("skip" = Option<usize>, Query, description = "Skip N muted users"),
        ("limit" = Option<usize>, Query, description = "Retrieve N muted users")
    ),
    responses(
        (status = 200, description = "User muted list", body = Muted),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_muted_handler(
    Path(user_id): Path<String>,
    Query(query): Query<Pagination>,
) -> Result<Json<Muted>> {
    info!("GET {USER_MUTED_ROUTE} user_id:{}", user_id);

    let skip = query.skip.unwrap_or(0);
    let limit = query.limit.unwrap_or(200);

    match Muted::get_by_id(&user_id, Some(skip), Some(limit)).await {
        Ok(Some(muted)) => Ok(Json(muted)),
        Ok(None) => Err(Error::UserNotFound { user_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(paths(user_muted_handler), components(schemas(Muted)))]
pub struct UserMutedApiDoc;
