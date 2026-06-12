use crate::routes::v0::endpoints::HOMESERVERS_ROUTE;
use crate::routes::AppState;
use crate::Result;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use nexus_common::models::homeserver::{Homeserver, HsBlacklist};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

#[derive(Serialize, Deserialize, Debug, ToSchema)]
pub struct HomeserversResponse {
    pub homeservers: Vec<String>,
}

#[utoipa::path(
    get,
    path = HOMESERVERS_ROUTE,
    tag = "Homeservers",
    responses(
        (
            status = 200,
            description = "Known non-blacklisted homeservers this Nexus can index once users are assigned",
            body = HomeserversResponse
        ),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn homeservers_handler(
    State(app_state): State<AppState>,
) -> Result<Json<HomeserversResponse>> {
    let homeservers = Homeserver::get_all_from_index().await?;
    let homeservers = filter_blacklisted_homeservers(homeservers, &app_state.hs_blacklist);

    Ok(Json(HomeserversResponse { homeservers }))
}

fn filter_blacklisted_homeservers(
    homeservers: Vec<String>,
    hs_blacklist: &HsBlacklist,
) -> Vec<String> {
    homeservers
        .into_iter()
        .filter(|hs_id| !hs_blacklist.is_blacklisted(hs_id))
        .collect()
}

pub fn routes() -> Router<AppState> {
    Router::new().route(HOMESERVERS_ROUTE, get(homeservers_handler))
}

#[derive(OpenApi)]
#[openapi(paths(homeservers_handler), components(schemas(HomeserversResponse)))]
pub struct HomeserversApiDoc;

#[cfg(test)]
mod tests {
    use super::*;
    use pubky::Keypair;
    use pubky_app_specs::PubkyId;

    #[test]
    fn test_filter_blacklisted_homeservers() {
        let allowed = PubkyId::from(Keypair::random().public_key());
        let blacklisted = PubkyId::from(Keypair::random().public_key());
        let hs_blacklist = HsBlacklist::new([blacklisted.clone()]);

        let homeservers = filter_blacklisted_homeservers(
            vec![allowed.to_string(), blacklisted.to_string()],
            &hs_blacklist,
        );

        assert_eq!(homeservers, vec![allowed.to_string()]);
    }
}
