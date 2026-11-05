use crate::models::PubkyId;
use crate::routes::v0::endpoints::BOOTSTRAP_ROUTE;
use crate::routes::v0::endpoints::INGEST_USER_ROUTE;
use crate::routes::v0::version::{versioned_handlers, ApiVersion};
use crate::routes::AppState;
use crate::routes::Path;
use crate::Result;

use axum::extract::State;
use axum::routing::put;
use axum::Json;
use axum::Router;
use nexus_common::models::bootstrap::{Bootstrap, ViewType};
use tracing::debug;
use utoipa::OpenApi;

/// The one body behind `bootstrap_handler_v0` and `bootstrap_handler_v1`: the payload in the
/// version's shape.
pub async fn bootstrap<V: ApiVersion>(Path(user_id): Path<PubkyId>) -> Result<Json<V::Bootstrap>> {
    debug!("GET {BOOTSTRAP_ROUTE}, user_id:{}", user_id);
    let bootstrap = Bootstrap::get_by_id(&user_id, ViewType::Full).await?;
    Ok(Json(V::Bootstrap::from(bootstrap)))
}

versioned_handlers! {
    bootstrap_handler_v0 / bootstrap_handler_v1 => bootstrap(path: Path<PubkyId>) -> shapes::Bootstrap;
    #[utoipa::path(
        get,
        path = BOOTSTRAP_ROUTE,
        operation_id = "bootstrap_handler",
        description = "Initial payload for all data required to bootstrap the pubky.app application. The client app will request it while the user is performing sign-in/sign-up in order to pre-populate the client DB",
        tag = "Bootstrap",
        params(
            ("user_id" = PubkyId, Path, description = "User Pubky ID")
        )
    )]
    responses(
        (status = 200, description = "Initial payload to bootstrap the client"),
        (status = 400, description = "Invalid parameters"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
}

#[utoipa::path(
    put,
    path = INGEST_USER_ROUTE,
    description = "Ingest a user by resolving their homeserver and persisting a user node in the graph. If the user is already known, this is a no-op.",
    tag = "Bootstrap",
    params(
        ("user_id" = PubkyId, Path, description = "User Pubky ID")
    ),
    responses(
        (status = 200, description = "User successfully ingested (or already known)"),
        (status = 400, description = "Invalid parameters"),
        (status = 403, description = "User is hosted on a blacklisted HS"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn ingest_user_handler(
    State(app_state): State<AppState>,
    Path(user_id): Path<PubkyId>,
) -> Result<()> {
    debug!("PUT {INGEST_USER_ROUTE}, user_id:{user_id}");

    app_state.ingestor.maybe_ingest_user(&user_id).await?;
    Ok(())
}

/// The shared bootstrap route and schema; the payload route comes with the version's document.
#[derive(OpenApi)]
#[openapi(paths(ingest_user_handler), components(schemas(PubkyId)))]
pub struct BootstrapApiDoc;

/// The payload route for one version, in the version's shape.
pub fn expensive_routes<V: ApiVersion>() -> Router<AppState> {
    Router::new().route(BOOTSTRAP_ROUTE, V::bootstrap())
}

pub fn routes() -> Router<AppState> {
    Router::new().route(INGEST_USER_ROUTE, put(ingest_user_handler))
}
