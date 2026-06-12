use crate::routes::v0::endpoints::HOMESERVERS_ROUTE;
use crate::routes::AppState;
use crate::Result;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use nexus_common::models::homeserver::{Homeserver, HsBlacklist};
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = HOMESERVERS_ROUTE,
    tag = "Homeservers",
    responses(
        (
            status = 200,
            description = "List known homeservers available as indexing sources",
            body = Vec<String>
        ),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn homeservers_handler(
    State(app_state): State<AppState>,
) -> Result<Json<Vec<String>>> {
    let homeservers = Homeserver::get_all_from_graph().await?;
    let homeservers = filter_allowed_homeservers(homeservers, &app_state.hs_blacklist);

    Ok(Json(homeservers))
}

fn filter_allowed_homeservers(
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
#[openapi(paths(homeservers_handler))]
pub struct HomeserversApiDoc;

#[cfg(test)]
mod tests {
    use super::*;
    use pubky::Keypair;
    use pubky_app_specs::PubkyId;

    #[test]
    fn test_filter_allowed_homeservers() {
        let allowed = PubkyId::from(Keypair::random().public_key());
        let blacklisted = PubkyId::from(Keypair::random().public_key());
        let hs_blacklist = HsBlacklist::new([blacklisted.clone()]);

        let homeservers = filter_allowed_homeservers(
            vec![allowed.to_string(), blacklisted.to_string()],
            &hs_blacklist,
        );

        assert_eq!(homeservers, vec![allowed.to_string()]);
    }
}
