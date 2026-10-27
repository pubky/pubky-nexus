use crate::models::{BoundedLimit, BoundedPagination, BoundedSkip, PubkyId};
use crate::routes::v0::endpoints::NOTIFICATION_ROUTE;
use crate::routes::v0::version::{versioned_handlers, ApiVersion};
use crate::routes::Path;
use crate::routes::Query;
use crate::Result;
use axum::http::HeaderValue;
use axum::response::{IntoResponse, Response};
use axum::Json;
use const_format::concatcp;
use nexus_common::models::notification::{
    Notification, NotificationType, PostChangedSource, NOTIFICATION_FILTER_MAX_SCAN,
};
use serde::Deserialize;
use tracing::debug;

/// Resume cursor for a type-filtered scan that hit its cap; see the handler docs.
pub const NEXT_START_HEADER: &str = "x-next-start";
use utoipa::OpenApi;

/// The query of `GET /user/{user_id}/notifications`, with `types` as version `V` reads it:
/// `/v0` the twelve types it has, `/v1` all thirteen.
#[derive(Deserialize)]
#[serde(bound(deserialize = ""))]
pub struct NotificationsQuery<V: ApiVersion> {
    #[serde(flatten)]
    pub pagination: BoundedPagination<10_000, 20, 100>,
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub types: Option<V::NotificationTypes>,
}

/// The one body behind `list_notifications_handler_v0` and `list_notifications_handler_v1`:
/// the page in the version's shape, with the resume cursor header when the scan stopped early.
pub async fn list_notifications<V: ApiVersion>(
    Path(user_id): Path<PubkyId>,
    Query(query): Query<NotificationsQuery<V>>,
) -> Result<Response> {
    debug!("GET {NOTIFICATION_ROUTE} for user_id: {}", user_id);

    let pagination = query.pagination.to_pagination(query.start, query.end);

    let (notifications, next_start) = match query.types {
        Some(types) => {
            let types: Vec<NotificationType> = types.into();
            Notification::get_by_id_filtered(&user_id, pagination, &types).await?
        }
        // `/v0` has no `event_changed`: it pages over the types it has, so one it leaves out
        // neither shortens a page nor counts in `skip`. The scan reaches `skip + limit` and the
        // usual cap beyond, so a history without such a notification pages exactly as the
        // unfiltered read below, however deep.
        None if !V::SOCIAL_EVENTS => {
            let max_scan = pagination.skip.unwrap_or(0)
                + pagination.limit.unwrap_or(20)
                + NOTIFICATION_FILTER_MAX_SCAN;
            Notification::get_by_id_filtered_capped(
                &user_id,
                pagination,
                &V0_NOTIFICATION_TYPES,
                max_scan,
            )
            .await?
        }
        None => {
            let notifications = Notification::get_by_id(&user_id, pagination).await?;
            return Ok(Json(in_version::<V>(notifications)).into_response());
        }
    };
    let mut response = Json(in_version::<V>(notifications)).into_response();
    if let Some(next_start) = next_start {
        response
            .headers_mut()
            .insert(NEXT_START_HEADER, HeaderValue::from(next_start));
    }
    Ok(response)
}

/// The notification types `/v0` has: every one but `event_changed`.
const V0_NOTIFICATION_TYPES: [NotificationType; 12] = [
    NotificationType::Follow,
    NotificationType::NewFriend,
    NotificationType::LostFriend,
    NotificationType::TagPost,
    NotificationType::TagProfile,
    NotificationType::UntagPost,
    NotificationType::UntagProfile,
    NotificationType::Reply,
    NotificationType::Repost,
    NotificationType::Mention,
    NotificationType::PostDeleted,
    NotificationType::PostEdited,
];

/// The page in the version's shape. `/v0` reads only the types it has; the filter keeps an
/// `event_changed` out of its page whatever read it came from.
fn in_version<V: ApiVersion>(notifications: Vec<Notification>) -> Vec<V::Notification> {
    notifications
        .into_iter()
        .filter(|notification| {
            V::SOCIAL_EVENTS
                || notification.body.notification_type() != NotificationType::EventChanged
        })
        .map(V::Notification::from)
        .collect()
}

versioned_handlers! {
    list_notifications_handler_v0 / list_notifications_handler_v1 => list_notifications(user_id: Path<PubkyId>, query: Query<shapes::NotificationsQuery>) -> Vec<shapes::Notification>;
    #[utoipa::path(
        get,
        path = NOTIFICATION_ROUTE,
        operation_id = "list_notifications_handler",
        tag = "User",
        description = concatcp!("List of user notifications", shapes::NOTIFICATION_LIST_RULE),
        params(
            ("user_id" = PubkyId, Path, description = "User Pubky ID"),
            ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Skip N notifications (max 10000)"),
            ("limit" = Option<BoundedLimit<20, 100>>, Query, description = "Retrieve N notifications (1–100, default 20)"),
            ("start" = Option<f64>, Query, description = "The start of the notifications timeframe. Notifications with a timestamp greater than this value will be excluded from the results"),
            ("end" = Option<f64>, Query, description = "The end of the notifications timeframe. Notifications with a timestamp less than this value will be excluded from the results"),
            ("types" = Option<shapes::NotificationTypes>, Query, description = concatcp!("Comma-separated notification types to keep (", shapes::NOTIFICATION_TYPES_RANGE, ", duplicates ignored), e.g. `&types=reply,mention`. Valid values are the `type` tags of the notification body: ", shapes::NOTIFICATION_TYPE_NAMES, "; anything else is rejected with 400. `skip` and `limit` then count matching notifications. At most 10,000 notifications are scanned per request; when that cap is hit before the page fills, the page can be short or empty and the `X-Next-Start` response header carries the `start` to resume from."))
        )
    )]
    // The header text is `/v1`'s (`V1::NEXT_START_HEADER_DOC`); `/v0` writes its own into its
    // document (`V0::finish_docs`), since it also sets the header without `types`.
    responses(
        (status = 200, description = "List of notifications", headers(("X-Next-Start" = i64, description = "Only with `types`: present when the scan stopped at its 10,000-entry cap before filling the page, so more matches may exist. Pass it as `start` to continue. Absent means the history was read to the end (or the page is full; page on from the last timestamp)."))),
        (status = 400, description = "Invalid parameters"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
}

/// The schemas a notification nests in every version; the notification itself and the
/// `types=` list are each version's.
#[derive(OpenApi)]
#[openapi(components(schemas(NotificationType, PostChangedSource, PubkyId)))]
pub struct NotificationsApiDocs;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::test_helpers::enum_values;
    use std::str::FromStr;

    /// `/v0` pages over every stored notification type but `event_changed`, each once: a type
    /// added later fails this test until it is placed on one side.
    #[test]
    fn v0_types_are_every_type_but_event_changed() {
        let stored: Vec<NotificationType> = enum_values::<NotificationType>()
            .iter()
            .map(|name| NotificationType::from_str(name).unwrap())
            .collect();
        let expected: Vec<NotificationType> = stored
            .into_iter()
            .filter(|t| *t != NotificationType::EventChanged)
            .collect();
        assert_eq!(V0_NOTIFICATION_TYPES.to_vec(), expected);
    }
}
