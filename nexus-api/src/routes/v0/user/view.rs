use crate::routes::v0::endpoints::{USER_ALIVE_ROUTE, USER_ROUTE};
use crate::routes::v0::user::types::ImAliveResponse;
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use nexus_common::models::tag::TagDetails;
use nexus_common::models::user::UserView;
use serde::Deserialize;
use tracing::info;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct ProfileQuery {
    viewer_id: Option<String>,
    depth: Option<u8>,
}

#[utoipa::path(
    get,
    path = USER_ROUTE,
    description = "User profile",
    tag = "User",
    params(
        ("user_id" = String, Path, description = "User Pubky ID"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("depth" = Option<usize>, Query, description = "User trusted network depth, user following users distance. Numbers bigger than 4, will be ignored")
    ),
    responses(
        (status = 200, description = "User Profile", body = UserView),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_view_handler(
    Path(user_id): Path<String>,
    Query(query): Query<ProfileQuery>,
) -> Result<Json<UserView>> {
    info!(
        "GET {USER_ROUTE} user_id:{}, viewer_id:{:?}, depth: {:?}",
        user_id, query.viewer_id, query.depth
    );

    match UserView::get_by_id(&user_id, query.viewer_id.as_deref(), query.depth).await {
        Ok(Some(user)) => Ok(Json(user)),
        Ok(None) => Err(Error::UserNotFound { user_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[utoipa::path(
    get,
    path = USER_ALIVE_ROUTE,
    description = "Im alive",
    tag = "User",
    params(
        ("user_id" = String, Path, description = "User Pubky ID")
    ),
    responses(
        (status = 200, description = "User Profile", body = ImAliveResponse),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_im_alive_handler(
    Path(user_id): Path<String>,
    // TODO: Might need to add param like "ViewType". There are some data that it would be too much to delete in the first go
    //Query(query): Query<Pub>,
) -> Result<Json<ImAliveResponse>> {
    info!("GET {USER_ALIVE_ROUTE} user_id:{}", user_id);

    // let view_type = ViewType::Full;

    // match ImAliveResponse::create(&user_id, view_type).await {
    //     Ok(result) => Ok(Json(result)),
    //     Err(source) => Err(Error::InternalServerError { source }),
    // }
    println!("Until new client, im_alive endpoint not available. NOTE: If you want to test, uncomment above code ;)");
    Ok(Json(ImAliveResponse::default()))
}

#[derive(OpenApi)]
#[openapi(
    paths(user_view_handler, user_im_alive_handler),
    components(schemas(UserView, TagDetails, ImAliveResponse))
)]
pub struct UserViewApiDoc;
