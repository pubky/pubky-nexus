use crate::register_routes;
use crate::routes::v0::endpoints;
use crate::routes::AppState;
use axum::Router;
use utoipa::OpenApi;

mod list;

pub fn routes() -> Router<AppState> {
    register_routes!(Router::new(),
        endpoints::NOTIFICATION_ROUTE => list::list_notifications_handler
    )
}

#[derive(OpenApi)]
#[openapi()]
pub struct NotificationApiDoc;

impl NotificationApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        list::NotificationsApiDocs::openapi()
    }
}
