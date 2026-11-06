use crate::models::PubkyId;
use crate::routes::v0::endpoints::USER_COUNTS_ROUTE;
use crate::routes::v0::version::{versioned_handlers, ApiVersion};
use crate::routes::Path;
use crate::{Error, Result};
use axum::Json;
use nexus_common::models::user::UserCounts;
use tracing::debug;
use utoipa::OpenApi;

/// The one body behind `user_counts_handler_v0` and `user_counts_handler_v1`: the counts in
/// the version's shape.
pub async fn user_counts<V: ApiVersion>(
    Path(user_id): Path<PubkyId>,
) -> Result<Json<V::UserCounts>> {
    debug!("GET {USER_COUNTS_ROUTE} user_id:{}", user_id);

    match UserCounts::get_by_id(&user_id).await? {
        Some(counts) => Ok(Json(V::UserCounts::from(counts))),
        None => Err(Error::user_not_found(user_id)),
    }
}

versioned_handlers! {
    user_counts_handler_v0 / user_counts_handler_v1 => user_counts(path: Path<PubkyId>) -> shapes::UserCounts;
    #[utoipa::path(
        get,
        path = USER_COUNTS_ROUTE,
        operation_id = "user_counts_handler",
        tag = "User",
        description = "User counts",
        params(
            ("user_id" = PubkyId, Path, description = "User Pubky ID")
        )
    )]
    responses(
        (status = 200, description = "User counts"),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "User not found"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
}

/// The schema the counts route nests in every version; the counts themselves are each
/// version's.
#[derive(OpenApi)]
#[openapi(components(schemas(PubkyId)))]
pub struct UserCountsApiDoc;
