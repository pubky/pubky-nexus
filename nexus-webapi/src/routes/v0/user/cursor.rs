use crate::models::PubkyId;
use crate::routes::v0::endpoints::USER_CURSOR_ROUTE;
use crate::routes::{AppState, Path};
use crate::{Error, Result};
use axum::extract::State;
use axum::Json;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::models::user::{get_user_homeserver, UserHsCursor};
use serde::Serialize;
use tracing::debug;
use utoipa::{OpenApi, ToSchema};

/// The user's event cursor for their current homeserver, as tracked by the indexer.
///
/// Users hosted on the primary homeserver are served the homeserver-level bulk
/// indexing cursor; users on external homeservers are served their per-user
/// cursor.
///
/// The cursor is `0` when the user is bound to a homeserver but no events have
/// been indexed for them yet.
#[derive(Debug, Serialize, ToSchema)]
pub struct UserCursorResponse {
    pub user_id: PubkyId,
    pub homeserver_id: String,
    pub cursor: u64,
}

#[utoipa::path(
    get,
    path = USER_CURSOR_ROUTE,
    tag = "User",
    description = "User homeserver event cursor",
    params(
        ("user_id" = PubkyId, Path, description = "User Pubky ID")
    ),
    responses(
        (status = 200, description = "User homeserver event cursor", body = UserCursorResponse),
        (status = 404, description = "User not found"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_cursor_handler(
    State(state): State<AppState>,
    Path(user_id): Path<PubkyId>,
) -> Result<Json<UserCursorResponse>> {
    debug!("GET {USER_CURSOR_ROUTE} user_id:{}", user_id);

    // 404 when the user is unknown or their homeserver is not resolved yet.
    let Some(homeserver_id) = get_user_homeserver(&user_id).await? else {
        return Err(Error::user_not_found(user_id));
    };

    let cursor = if homeserver_id.as_str() == state.primary_homeserver.as_ref() {
        // The primary homeserver is bulk-indexed, so its position is tracked at
        // the homeserver level, not per user. `0` means nothing indexed yet.
        Homeserver::get_from_index(&homeserver_id)
            .await?
            .map(|hs| hs.cursor)
            .unwrap_or_default()
    } else {
        UserHsCursor::read(&[user_id.as_ref()], &homeserver_id)
            .await?
            .into_iter()
            .next()
            .unwrap_or_default()
    };

    Ok(Json(UserCursorResponse {
        user_id,
        homeserver_id,
        cursor,
    }))
}

#[derive(OpenApi)]
#[openapi(
    paths(user_cursor_handler),
    components(schemas(UserCursorResponse, PubkyId))
)]
pub struct UserCursorApiDoc;
