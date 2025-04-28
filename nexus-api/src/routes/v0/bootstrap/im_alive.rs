use crate::routes::v0::endpoints::IM_ALIVE_ROUTE;
use crate::Result;
use axum::extract::Path;
use axum::Json;
use nexus_common::models::bootstrap::Bootstrap;
use tracing::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = IM_ALIVE_ROUTE,
    description = "Initial payload of all data required to bootstrap the pubky.app application. The client will request in the authentication process",
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
    info!("GET {IM_ALIVE_ROUTE}, user_id:{}", user_id);

    // let view_type = ViewType::Full;

    // match Bootstrap::build(&user_id, view_type).await {
    //     Ok(result) => Ok(Json(result)),
    //     Err(source) => Err(Error::InternalServerError { source }),
    // }
    println!("Until new client, im_alive endpoint not available. NOTE: If you want to test, uncomment above code ;)");
    Ok(Json(Bootstrap::default()))
}

#[derive(OpenApi)]
#[openapi(paths(user_bootstrap_handler), components(schemas(Bootstrap)))]
pub struct ImAliveApiDoc;
