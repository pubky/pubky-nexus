use crate::models::user::{Follows, FollowsVariant};
use crate::routes::v0::endpoints::USER_FOLLOWERS_ROUTE;
use crate::{Error, Result};
use axum::extract::Path;
use axum::Json;
use log::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = USER_FOLLOWERS_ROUTE,
    tag = "User Followers List",
    params(
        ("user_id" = String, Path, description = "User Pubky ID")
    ),
    responses(
        (status = 200, description = "User details", body = Follows),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_followers_handler(Path(user_id): Path<String>) -> Result<Json<Follows>> {
    info!("GET {USER_FOLLOWERS_ROUTE} user_id:{}", user_id);

    match Follows::get_by_id(&user_id, FollowsVariant::Followers).await {
        Ok(Some(followers)) => Ok(Json(followers)),
        Ok(None) => Err(Error::UserNotFound { user_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(paths(user_followers_handler), components(schemas(Follows)))]
pub struct UserFollowersApiDoc;
