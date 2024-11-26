use crate::models::notification::{Notification, NotificationBody, PostChangedSource};
use crate::routes::v0::endpoints::NOTIFICATION_ROUTE;
use crate::types::Pagination;
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use log::info;
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
        ("start" = Option<String>, Query, description = "Start timestamp for notification retrieval"),
        ("end" = Option<String>, Query, description = "End timestamp for notification retrieval")
    ),
    responses(
        (status = 200, description = "List of notifications", body = Vec<Notification>),
        (status = 404, description = "No notifications found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_notifications_handler(
    Path(user_id): axum::extract::Path<String>,
    Query(pagination): Query<Pagination>,
) -> Result<Json<Vec<Notification>>> {
    info!("GET {NOTIFICATION_ROUTE} for user_id: {}", user_id);

    match Notification::get_by_id(&user_id, pagination).await {
        Ok(notifications) => Ok(Json(notifications)),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(list_notifications_handler,),
    components(schemas(Notification, NotificationBody, PostChangedSource))
)]
pub struct NotificationsApiDocs;
