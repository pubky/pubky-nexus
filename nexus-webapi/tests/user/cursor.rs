use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;
use nexus_common::config::watcher::HOMESERVER_PUBKY;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::models::traits::Collection;
use nexus_common::models::user::{set_user_homeserver, UserDetails, UserHsCursor};
use nexus_common::utils::test_utils::random_pubky_id;
use pubky_app_specs::PubkyId;

async fn env_init() {
    crate::utils::server::TestServiceServer::get_test_server().await;
}

/// Seeds a user in the graph, binds them to a homeserver, and advances their HS
/// cursor. The endpoint must return the bound homeserver and cursor value.
#[tokio_shared_rt::test(shared)]
async fn test_user_cursor_returns_bound_homeserver_and_cursor() -> Result<()> {
    env_init().await;

    let user_id = random_pubky_id();
    let hs_id = random_pubky_id();
    let user_id_str = user_id.to_string();
    let hs_id_str = hs_id.to_string();

    UserDetails::from_pubky(user_id.clone())
        .put_to_graph()
        .await?;
    set_user_homeserver(&user_id_str, &hs_id_str).await?;
    UserHsCursor::write(&user_id_str, &hs_id_str, 42).await?;

    let res = get_request(&format!("/v0/user/{user_id_str}/cursor")).await?;
    assert_eq!(res["user_id"], user_id_str);
    assert_eq!(res["homeserver_id"], hs_id_str);
    assert_eq!(res["cursor"], 42);

    Ok(())
}

/// A user bound to a homeserver but with no cursor written yet reads back as 0.
#[tokio_shared_rt::test(shared)]
async fn test_user_cursor_defaults_to_zero() -> Result<()> {
    env_init().await;

    let user_id = random_pubky_id();
    let hs_id = random_pubky_id();
    let user_id_str = user_id.to_string();
    let hs_id_str = hs_id.to_string();

    UserDetails::from_pubky(user_id.clone())
        .put_to_graph()
        .await?;
    set_user_homeserver(&user_id_str, &hs_id_str).await?;

    let res = get_request(&format!("/v0/user/{user_id_str}/cursor")).await?;
    assert_eq!(res["homeserver_id"], hs_id_str);
    assert_eq!(res["cursor"], 0);

    Ok(())
}

/// A user without a homeserver binding is not found.
#[tokio_shared_rt::test(shared)]
async fn test_user_cursor_not_found() -> Result<()> {
    env_init().await;

    let user_id = random_pubky_id();
    invalid_get_request(&format!("/v0/user/{user_id}/cursor"), StatusCode::NOT_FOUND).await?;

    Ok(())
}

/// A malformed user id is rejected with 400.
#[tokio_shared_rt::test(shared)]
async fn test_user_cursor_invalid_user_id() -> Result<()> {
    env_init().await;

    invalid_get_request("/v0/user/not-a-pubky-id/cursor", StatusCode::BAD_REQUEST).await?;

    Ok(())
}

/// A user hosted on the primary homeserver is served the homeserver-level index
/// cursor, which takes precedence over any stale per-user cursor entry.
#[tokio_shared_rt::test(shared)]
async fn test_user_cursor_primary_homeserver_returns_hs_cursor() -> Result<()> {
    env_init().await;

    let user_id = random_pubky_id();
    let hs_id = PubkyId::try_from(HOMESERVER_PUBKY).unwrap();
    let user_id_str = user_id.to_string();
    let hs_id_str = hs_id.to_string();

    // The homeserver cursor as it stands before this test advances it; the
    // endpoint must reflect it for a primary-homeserver user.
    let hs_cursor_before = Homeserver::get_from_index(&hs_id_str)
        .await?
        .map(|hs| hs.cursor)
        .unwrap_or_default();

    UserDetails::from_pubky(user_id.clone())
        .put_to_graph()
        .await?;
    set_user_homeserver(&user_id_str, &hs_id_str).await?;
    // A stale per-user entry must not shadow the homeserver-level cursor.
    UserHsCursor::write(&user_id_str, &hs_id_str, 7).await?;

    let res = get_request(&format!("/v0/user/{user_id_str}/cursor")).await?;
    assert_eq!(res["homeserver_id"], hs_id_str);
    assert_eq!(res["cursor"], hs_cursor_before);

    // Advance the homeserver cursor; the endpoint must follow it.
    let target = hs_cursor_before + 1234;
    Homeserver::try_from_cursor(hs_id.clone(), target.to_string())
        .await?
        .put_to_index()
        .await?;

    let res = get_request(&format!("/v0/user/{user_id_str}/cursor")).await?;
    assert_eq!(res["cursor"], target);

    Ok(())
}
