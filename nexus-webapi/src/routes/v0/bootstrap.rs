use crate::routes::v0::endpoints::{self, PUT_HOMESERVER_ROUTE};
use crate::routes::AppState;
use crate::Result;
use crate::{routes::v0::endpoints::BOOTSTRAP_ROUTE, Error};

use axum::extract::Path;
use axum::routing::{get, put};
use axum::Json;
use axum::Router;
use nexus_common::models::bootstrap::{Bootstrap, ViewType};
use nexus_common::models::homeserver::Homeserver;
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
    description = "Start monitoring Pubky App data on a new homeserver",
    tag = "Bootstrap",
    params(
        ("homeserver_pk" = String, Path, description = "Homeserver PK")
    ),
    responses(
        (status = 200, description = "Successfully added new homeserver"),
        (status = 404, description = "No homeserver found with the specified PK"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn put_homeserver_handler(Path(homeserver_pk): Path<String>) -> Result<()> {
    info!("PUT {PUT_HOMESERVER_ROUTE}, homeserver_pk:{homeserver_pk}");

    let hs = PubkyId::try_from(&homeserver_pk)
        .map(Homeserver::new)
        .map_err(|e| Error::invalid_input(&format!("Invalid homeserver PK: {e}")))?;

    // Before saving to graph, check if it exists (indexed)
    match Homeserver::get_from_index(&homeserver_pk).await {
        Err(e) => Err(Error::internal(e)),
        Ok(Some(_)) => Err(Error::invalid_input("Homeserver is already known")),
        Ok(None) => {
            hs.put_to_graph()
                .await
                .map_err(|_| Error::invalid_input("Failed to store homeserver to graph"))?;
            hs.put_to_index()
                .await
                .map_err(|_| Error::invalid_input("Failed to add homeserver to index"))
        }
    }
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
