use crate::models::{BoundedLimit, BoundedPagination, BoundedSkip, NotificationTypes, PubkyId};
use crate::routes::v0::endpoints::NOTIFICATION_ROUTE;
use crate::routes::Path;
use crate::routes::Query;
use crate::Result;
use axum::http::HeaderValue;
use axum::response::{IntoResponse, Response};
use axum::Json;
use nexus_common::models::notification::{
    Notification, NotificationBody, NotificationType, PostChangedSource,
};
use pubky_social_specs::legacy_v0::PubkyAppPostKind;
use serde::Deserialize;
use tracing::debug;

/// Resume cursor for a type-filtered scan that hit its cap; see the handler docs.
pub const NEXT_START_HEADER: &str = "x-next-start";
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct NotificationsQuery {
    #[serde(flatten)]
    pub pagination: BoundedPagination<10_000, 20, 100>,
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub types: Option<NotificationTypes>,
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
        ("end" = Option<f64>, Query, description = "The end of the notifications timeframe. Notifications with a timestamp less than this value will be excluded from the results"),
        ("types" = Option<NotificationTypes>, Query, description = "Comma-separated notification types to keep (1-12, duplicates ignored), e.g. `&types=reply,mention`. Valid values are the `type` tags of the notification body: follow, new_friend, lost_friend, tag_post, tag_profile, untag_post, untag_profile, reply, repost, mention, post_deleted, post_edited; anything else is rejected with 400. `skip` and `limit` then count matching notifications. At most 10,000 notifications are scanned per request; when that cap is hit before the page fills, the page can be short or empty and the `X-Next-Start` response header carries the `start` to resume from.")
    ),
    responses(
        (status = 200, description = "List of notifications", body = Vec<Notification>, headers(("X-Next-Start" = i64, description = "Only with `types`: present when the scan stopped at its 10,000-entry cap before filling the page, so more matches may exist. Pass it as `start` to continue. Absent means the history was read to the end (or the page is full; page on from the last timestamp)."))),
        (status = 400, description = "Invalid parameters"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_notifications_handler(
    Path(user_id): Path<PubkyId>,
    Query(query): Query<NotificationsQuery>,
) -> Result<Response> {
    debug!("GET {NOTIFICATION_ROUTE} for user_id: {}", user_id);

    let pagination = query.pagination.to_pagination(query.start, query.end);

    let Some(types) = query.types else {
        let notifications = Notification::get_by_id(&user_id, pagination).await?;
        return Ok(Json(notifications).into_response());
    };

    let (notifications, next_start) =
        Notification::get_by_id_filtered(&user_id, pagination, &types.0).await?;
    let mut response = Json(notifications).into_response();
    if let Some(next_start) = next_start {
        response
            .headers_mut()
            .insert(NEXT_START_HEADER, HeaderValue::from(next_start));
    }
    Ok(response)
}

#[derive(OpenApi)]
#[openapi(
    paths(list_notifications_handler,),
    components(schemas(
        Notification,
        NotificationBody,
        NotificationType,
        NotificationTypes,
        PostChangedSource,
        PubkyAppPostKind,
        PubkyId
    ))
)]
pub struct NotificationsApiDocs;
