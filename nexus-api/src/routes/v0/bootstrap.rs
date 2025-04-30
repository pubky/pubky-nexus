use crate::register_routes;
use crate::routes::v0::endpoints;
use crate::routes::AppState;
use crate::Result;
use crate::{routes::v0::endpoints::BOOTSTRAP_ROUTE, Error};
use axum::extract::Path;
use axum::Json;
use axum::Router;
use nexus_common::models::bootstrap::{Bootstrap, ViewType};
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

#[derive(OpenApi)]
#[openapi(paths(bootstrap_handler), components(schemas(Bootstrap)))]
pub struct BootstrapApiDoc;

pub fn routes() -> Router<AppState> {
    register_routes!(Router::new(),
        endpoints::BOOTSTRAP_ROUTE => bootstrap_handler,
    )
}
