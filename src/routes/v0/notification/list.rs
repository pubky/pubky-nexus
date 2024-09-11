use crate::models::notification::{Notification, NotificationBody, PostDeleteType};
use crate::routes::v0::endpoints::NOTIFICATION_ROUTE;
use crate::{Error, Result};
use axum::extract::Query;
use axum::Json;
use log::info;
use serde::Deserialize;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct NotificationQuery {
    skip: Option<usize>,
    limit: Option<usize>,
    start: Option<f64>, // Start timestamp (optional)
    end: Option<f64>,   // End timestamp (optional)
}

#[utoipa::path(
    get,
    path = NOTIFICATION_ROUTE,
    tag = "Notifications",
    params(
        ("user_id" = String, Path, description = "User Pubky ID"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
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
    Query(query): Query<NotificationQuery>,
    axum::extract::Path(user_id): axum::extract::Path<String>,
) -> Result<Json<Vec<Notification>>> {
    info!("GET {NOTIFICATION_ROUTE} for user_id: {}", user_id);

    let skip = query.skip.unwrap_or(0);
    let limit = query.limit.unwrap_or(20);

    match Notification::list(&user_id, Some(limit), Some(skip), query.start, query.end).await {
        Ok(notifications) => {
            if notifications.is_empty() {
                Err(Error::UserNotFound {
                    user_id: user_id.to_string(),
                })
            } else {
                Ok(Json(notifications))
            }
        }
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(list_notifications_handler,),
    components(schemas(Notification, NotificationBody, PostDeleteType))
)]
pub struct NotificationsApiDocs;
