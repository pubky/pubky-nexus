use crate::models::tag::user::UserTags;
use crate::models::tag::TagDetails;
use crate::models::user::UserView;
use crate::routes::v0::endpoints::USER_ROUTE;
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use log::info;
use serde::Deserialize;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct ProfileQuery {
    viewer_id: Option<String>,
}

#[utoipa::path(
    get,
    path = USER_ROUTE,
    tag = "User Profile",
    params(
        ("user_id" = String, Path, description = "User Pubky ID"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID")
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
        "GET {USER_ROUTE} user_id:{}, viewer_id:{:?}",
        user_id, query.viewer_id
    );

    match UserView::get_by_id(&user_id, query.viewer_id.as_deref()).await {
        Ok(Some(user)) => Ok(Json(user)),
        Ok(None) => Err(Error::UserNotFound { user_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(user_view_handler),
    components(schemas(UserView, UserTags, TagDetails))
)]
pub struct UserViewApiDoc;
