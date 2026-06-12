use crate::utils::get_request;

use anyhow::Result;
use nexus_common::models::homeserver::Homeserver;
use nexus_webapi::routes::v0::endpoints::HOMESERVERS_ROUTE;
use pubky::Keypair;
use pubky_app_specs::PubkyId;
use std::collections::BTreeSet;

async fn create_homeserver_in_graph() -> Result<PubkyId> {
    let hs_id = PubkyId::from(Keypair::random().public_key());
    Homeserver::new(hs_id.clone()).put_to_graph().await?;
    Ok(hs_id)
}

#[tokio_shared_rt::test(shared)]
async fn test_homeservers_endpoint_returns_array() -> Result<()> {
    let body = get_request(HOMESERVERS_ROUTE).await?;

    assert!(
        body.as_array().is_some(),
        "response body should be a homeserver array"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_homeservers_endpoint_includes_graph_homeserver_without_users() -> Result<()> {
    let hs_id = create_homeserver_in_graph().await?;

    let homeservers = get_request(HOMESERVERS_ROUTE)
        .await?
        .as_array()
        .expect("response body should be a homeserver array")
        .clone();

    assert!(
        homeservers
            .iter()
            .any(|value| value.as_str() == Some(hs_id.as_ref())),
        "graph homeserver without assigned users should be returned"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_homeservers_endpoint_returns_created_graph_homeservers() -> Result<()> {
    let hs_ids = [
        create_homeserver_in_graph().await?,
        create_homeserver_in_graph().await?,
        create_homeserver_in_graph().await?,
    ];

    let expected_hs_ids = hs_ids
        .iter()
        .map(ToString::to_string)
        .collect::<BTreeSet<_>>();

    let returned_hs_ids = get_request(HOMESERVERS_ROUTE)
        .await?
        .as_array()
        .expect("response body should be a homeserver array")
        .iter()
        .filter_map(|value| value.as_str())
        .filter(|hs_id| expected_hs_ids.contains(*hs_id))
        .map(ToString::to_string)
        .collect::<BTreeSet<_>>();

    assert_eq!(returned_hs_ids, expected_hs_ids);

    Ok(())
}
