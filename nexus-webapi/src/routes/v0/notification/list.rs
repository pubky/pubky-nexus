use crate::models::{BoundedLimit, BoundedPagination, BoundedSkip, PubkyId};
use crate::routes::v0::endpoints::NOTIFICATION_ROUTE;
use crate::routes::Path;
use crate::routes::Query;
use crate::Result;
use axum::Json;
use nexus_common::models::notification::{Notification, NotificationBody, PostChangedSource};
use serde::Deserialize;
use tracing::debug;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct NotificationsQuery {
    #[serde(flatten)]
    pub pagination: BoundedPagination<10_000, 20, 100>,
    pub start: Option<f64>,
    pub end: Option<f64>,
}

#[utoipa::path(
    get,
    path = NOTIFICATION_ROUTE,
    tag = "User",
    description = "List of user notifications",
    params(
        ("user_id" = PubkyId, Path, description = "User Pubky ID"),
        ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Skip N notifications (max 10000)"),
        ("limit" = Option<BoundedLimit<20, 100>>, Query, description = "Retrieve N notifications (1–100, default 20)"),
        ("start" = Option<f64>, Query, description = "The start of the notifications timeframe. Notifications with a timestamp greater than this value will be excluded from the results"),
        ("end" = Option<f64>, Query, description = "The end of the notifications timeframe. Notifications with a timestamp less than this value will be excluded from the results")
    ),
    responses(
        (status = 200, description = "List of notifications", body = Vec<Notification>),
        (status = 400, description = "Invalid parameters"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_notifications_handler(
    Path(user_id): Path<PubkyId>,
    Query(query): Query<NotificationsQuery>,
) -> Result<Json<Vec<Notification>>> {
    debug!("GET {NOTIFICATION_ROUTE} for user_id: {}", user_id);

    let pagination = query.pagination.to_pagination(query.start, query.end);

    Ok(Json(Notification::get_by_id(&user_id, pagination).await?))
}

#[derive(OpenApi)]
#[openapi(
    paths(list_notifications_handler,),
    components(schemas(Notification, NotificationBody, PostChangedSource, PubkyId))
)]
pub struct NotificationsApiDocs;
