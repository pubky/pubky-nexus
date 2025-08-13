use crate::routes::v0::endpoints::NOTIFICATION_ROUTE;
use crate::routes::AppState;

use axum::routing::get;
use axum::Router;
use utoipa::OpenApi;

mod list;

pub fn routes() -> Router<AppState> {
    Router::new().route(NOTIFICATION_ROUTE, get(list::list_notifications_handler))
}

#[derive(OpenApi)]
#[openapi()]
pub struct NotificationApiDoc;

impl NotificationApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        list::NotificationsApiDocs::openapi()
    }
}
