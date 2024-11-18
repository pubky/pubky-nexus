use crate::models::pubky_app::UserLink;
use crate::models::user::UserDetails;
use crate::routes::v0::endpoints::USER_DETAILS_ROUTE;
use crate::types::PubkyId;
use crate::{Error, Result};
use axum::extract::Path;
use axum::Json;
use log::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = USER_DETAILS_ROUTE,
    tag = "User Details",
    params(
        ("user_id" = String, Path, description = "User Pubky ID")
    ),
    responses(
        (status = 200, description = "User details", body = UserDetails),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_details_handler(Path(user_id): Path<String>) -> Result<Json<UserDetails>> {
    info!("GET {USER_DETAILS_ROUTE} user_id:{}", user_id);

    match UserDetails::get_by_id(&user_id).await {
        Ok(Some(details)) => Ok(Json(details)),
        Ok(None) => Err(Error::UserNotFound { user_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(user_details_handler),
    components(schemas(UserDetails, UserLink, PubkyId))
)]
pub struct UserDetailsApiDoc;
