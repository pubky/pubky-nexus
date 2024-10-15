use crate::routes::v0::endpoints;
use crate::{register_routes, to_axum};
use axum::Router;
use utoipa::OpenApi;

mod list;

pub fn routes() -> Router {
    register_routes!(Router::new(),
        to_axum!(endpoints::NOTIFICATION_ROUTE) => list::list_notifications_handler
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
