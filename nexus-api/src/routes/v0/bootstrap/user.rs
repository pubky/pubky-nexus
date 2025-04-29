use crate::Result;
use crate::{routes::v0::endpoints::USER_BOOTSTRAP_ROUTE, Error};
use axum::extract::Path;
use axum::Json;
use nexus_common::models::bootstrap::{Bootstrap, ViewType};
use tracing::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = USER_BOOTSTRAP_ROUTE,
    description = "Initial payload for all data required to bootstrap the pubky.app application. The client app will request it while the user is performing sign-in in order to pre-populate the client DB",
    tag = "User",
    params(
        ("user_id" = String, Path, description = "User Pubky ID")
    ),
    responses(
        (status = 200, description = "Initial payload to bootstrap the client", body = Bootstrap),
        (status = 404, description = "User payload not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_bootstrap_handler(
    Path(user_id): Path<String>,
    // TODO: Might need to add param like "ViewType". There are some data that it would be too much to delete in the first go
    //Query(query): Query<Pub>,
) -> Result<Json<Bootstrap>> {
    info!("GET {USER_BOOTSTRAP_ROUTE}, user_id:{}", user_id);

    let view_type = ViewType::Full;

    match Bootstrap::build(&user_id, view_type).await {
        Ok(result) => Ok(Json(result)),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(paths(user_bootstrap_handler), components(schemas(Bootstrap)))]
pub struct ImAliveApiDoc;
