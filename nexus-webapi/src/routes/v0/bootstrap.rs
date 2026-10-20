use crate::models::PubkyId;
use crate::routes::v0::endpoints::BOOTSTRAP_ROUTE;
use crate::routes::v0::endpoints::INGEST_USER_ROUTE;
use crate::routes::v0::endpoints::RECHECK_USER_ROUTE;
use crate::routes::AppState;
use crate::routes::Path;
use crate::{Error, Result};

use axum::extract::State;
use axum::routing::{get, put};
use axum::Json;
use axum::Router;
use nexus_common::models::bootstrap::{Bootstrap, ViewType};
use nexus_common::models::user::{HomeserverRecheck, RecheckOutcome};
use tracing::debug;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = BOOTSTRAP_ROUTE,
    description = "Initial payload for all data required to bootstrap the pubky.app application. The client app will request it while the user is performing sign-in/sign-up in order to pre-populate the client DB",
    tag = "Bootstrap",
    params(
        ("user_id" = PubkyId, Path, description = "User Pubky ID")
    ),
    responses(
        (status = 200, description = "Initial payload to bootstrap the client", body = Bootstrap),
        (status = 400, description = "Invalid parameters"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn bootstrap_handler(Path(user_id): Path<PubkyId>) -> Result<Json<Bootstrap>> {
    debug!("GET {BOOTSTRAP_ROUTE}, user_id:{}", user_id);
    Ok(Json(Bootstrap::get_by_id(&user_id, ViewType::Full).await?))
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

#[utoipa::path(
    put,
    path = RECHECK_USER_ROUTE,
    description = "Re-check a known user's homeserver against the network now and follow the record: a record naming another homeserver moves the user there, so a user who migrated is indexed from the new homeserver without waiting for the periodic resolver. The client app should call it after a sign-in.",
    tag = "Bootstrap",
    params(
        ("user_id" = PubkyId, Path, description = "User Pubky ID")
    ),
    responses(
        (status = 200, description = "The user's homeserver after the check and what the check did", body = HomeserverRecheck),
        (status = 403, description = "User is hosted on a blacklisted HS"),
        (status = 404, description = "User not known to Nexus; ingest first"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn recheck_user_handler(
    State(app_state): State<AppState>,
    Path(user_id): Path<PubkyId>,
) -> Result<Json<HomeserverRecheck>> {
    debug!("PUT {RECHECK_USER_ROUTE}, user_id:{user_id}");

    match app_state.ingestor.recheck_user(&user_id).await? {
        Some(recheck) => Ok(Json(recheck)),
        None => Err(Error::user_not_found(user_id)),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(bootstrap_handler, ingest_user_handler, recheck_user_handler),
    components(schemas(Bootstrap, PubkyId, HomeserverRecheck, RecheckOutcome))
)]
pub struct BootstrapApiDoc;

pub fn expensive_routes() -> Router<AppState> {
    Router::new().route(BOOTSTRAP_ROUTE, get(bootstrap_handler))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(INGEST_USER_ROUTE, put(ingest_user_handler))
        .route(RECHECK_USER_ROUTE, put(recheck_user_handler))
}
