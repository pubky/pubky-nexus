use crate::routes::v0::endpoints::NOTIFICATION_ROUTE;
use crate::Result;
use axum::extract::{Path, Query};
use axum::Json;
use nexus_common::models::notification::{Notification, NotificationBody, PostChangedSource};
use nexus_common::types::Pagination;
use tracing::debug;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = NOTIFICATION_ROUTE,
    tag = "User",
    description = "List of user notifications",
    params(
        ("user_id" = String, Path, description = "User Pubky ID"),
        ("skip" = Option<usize>, Query, description = "Skip N notifications"),
        ("limit" = Option<usize>, Query, description = "Retrieve N notifications"),
        ("start" = Option<String>, Query, description = "The start of the notifications timeframe. Notifications with a timestamp greater than this value will be excluded from the results"),
        ("end" = Option<String>, Query, description = "The end of the notifications timeframe. Notifications with a timestamp less than this value will be excluded from the results")
    ),
    responses(
        (status = 200, description = "List of notifications", body = Vec<Notification>),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_notifications_handler(
    Path(user_id): axum::extract::Path<String>,
    Query(pagination): Query<Pagination>,
) -> Result<Json<Vec<Notification>>> {
    debug!("GET {NOTIFICATION_ROUTE} for user_id: {}", user_id);

    Ok(Json(Notification::get_by_id(&user_id, pagination).await?))
}

#[derive(OpenApi)]
#[openapi(
    paths(list_notifications_handler,),
    components(schemas(Notification, NotificationBody, PostChangedSource))
)]
pub struct NotificationsApiDocs;
