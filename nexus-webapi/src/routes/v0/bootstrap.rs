use crate::routes::v0::endpoints::{self, BOOTSTRAP_ROUTE, INGEST_USER_ROUTE};
use crate::routes::AppState;
use crate::{Error, Result};

use axum::extract::Path;
use axum::routing::{get, put};
use axum::Json;
use axum::Router;
use nexus_common::models::bootstrap::{Bootstrap, ViewType};
use nexus_common::models::user::UserDetails;
use pubky_app_specs::PubkyId;
use tracing::debug;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = BOOTSTRAP_ROUTE,
    description = "Initial payload for all data required to bootstrap the pubky.app application. The client app will request it while the user is performing sign-in/sign-up in order to pre-populate the client DB",
    tag = "Bootstrap",
    params(
        ("user_id" = String, Path, description = "User Pubky ID")
    ),
    responses(
        (status = 200, description = "Initial payload to bootstrap the client", body = Bootstrap),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn bootstrap_handler(
    Path(user_id): Path<String>,
    // TODO: Might need a param like "ViewType". There might be too much data to include in the first go, especially for mobile
    //Query(query): Query<Pub>,
) -> Result<Json<Bootstrap>> {
    debug!("GET {BOOTSTRAP_ROUTE}, user_id:{}", user_id);

    Ok(Json(Bootstrap::get_by_id(&user_id, ViewType::Full).await?))
}

#[utoipa::path(
    put,
    path = INGEST_USER_ROUTE,
    description = "Ingest a user by resolving their homeserver and persisting a user node in the graph. If the user is already known, this is a no-op.",
    tag = "Bootstrap",
    params(
        ("user_id" = String, Path, description = "User Pubky ID")
    ),
    responses(
        (status = 200, description = "User successfully ingested (or already known)"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn ingest_user_handler(Path(user_id): Path<String>) -> Result<()> {
    debug!("PUT {INGEST_USER_ROUTE}, user_id:{user_id}");

    PubkyId::try_from(&user_id)
        .map_err(|e| Error::invalid_input(&format!("Invalid user PK: {e}")))?;

    UserDetails::maybe_ingest_user(&user_id).await?;
    Ok(())
}

#[derive(OpenApi)]
#[openapi(
    paths(bootstrap_handler, ingest_user_handler),
    components(schemas(Bootstrap))
)]
pub struct BootstrapApiDoc;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(endpoints::BOOTSTRAP_ROUTE, get(bootstrap_handler))
        .route(endpoints::INGEST_USER_ROUTE, put(ingest_user_handler))
}
