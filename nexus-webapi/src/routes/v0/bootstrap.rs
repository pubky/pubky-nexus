use crate::routes::v0::endpoints::{self, PUT_HOMESERVER_ROUTE};
use crate::routes::AppState;
use crate::Result;
use crate::{routes::v0::endpoints::BOOTSTRAP_ROUTE, Error};

use axum::extract::Path;
use axum::routing::{get, put};
use axum::Json;
use axum::Router;
use nexus_common::models::bootstrap::{Bootstrap, ViewType};
use nexus_common::HomeserverManager;
use pubky_app_specs::PubkyId;
use tracing::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = BOOTSTRAP_ROUTE,
    description = "Initial payload for all data required to bootstrap the pubky.app application. The client app will request it while the user is performing sign-in in order to pre-populate the client DB",
    tag = "Bootstrap",
    params(
        ("user_id" = String, Path, description = "User Pubky ID")
    ),
    responses(
        (status = 200, description = "Initial payload to bootstrap the client", body = Bootstrap),
        (status = 404, description = "user_id requested for bootstrap payload not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn bootstrap_handler(
    Path(user_id): Path<String>,
    // TODO: Might need a param like "ViewType". There might be too much data to include in the first go, especially for mobile
    //Query(query): Query<Pub>,
) -> Result<Json<Bootstrap>> {
    info!("GET {BOOTSTRAP_ROUTE}, user_id:{}", user_id);

    match Bootstrap::get_by_id(&user_id, ViewType::Full).await {
        Ok(Some(result)) => Ok(Json(result)),
        Ok(None) => Err(Error::UserNotFound { user_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[utoipa::path(
    put,
    path = PUT_HOMESERVER_ROUTE,
    description = "Ingest (start monitoring all events of) the Homeserver on which this User PK stores data at this time",
    tag = "Bootstrap",
    params(
        ("user_id" = String, Path, description = "User Pubky ID")
    ),
    responses(
        (status = 200, description = "Successfully added new homeserver"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn put_homeserver_handler(Path(user_id): Path<String>) -> Result<()> {
    info!("PUT {PUT_HOMESERVER_ROUTE}, user_id:{user_id}");

    PubkyId::try_from(&user_id)
        .map_err(|e| Error::invalid_input(&format!("Invalid user PK: {e}")))?;

    HomeserverManager::maybe_ingest_for_user(&user_id)
        .await
        .map_err(Error::internal)
}

#[derive(OpenApi)]
#[openapi(
    paths(bootstrap_handler, put_homeserver_handler),
    components(schemas(Bootstrap))
)]
pub struct BootstrapApiDoc;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(endpoints::BOOTSTRAP_ROUTE, get(bootstrap_handler))
        .route(endpoints::PUT_HOMESERVER_ROUTE, put(put_homeserver_handler))
}
