use crate::routes::v0::endpoints::NOTIFICATION_ROUTE;
use crate::routes::v0::version::ApiVersion;
use crate::routes::AppState;

use axum::Router;
use utoipa::OpenApi;

pub mod list;

pub use list::NEXT_START_HEADER;

/// The notification route for one version, listing the version's notification shape.
pub fn routes<V: ApiVersion>() -> Router<AppState> {
    Router::new().route(NOTIFICATION_ROUTE, V::list_notifications())
}

#[derive(OpenApi)]
#[openapi()]
pub struct NotificationApiDoc;

impl NotificationApiDoc {
    /// The shared schemas; the route comes with the version's document.
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        list::NotificationsApiDocs::openapi()
    }
}
