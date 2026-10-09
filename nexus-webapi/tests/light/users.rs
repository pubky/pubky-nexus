use super::{assert_unavailable, light_request, setup_light_stack};
use anyhow::Result;
use axum::http::{Method, StatusCode};
use chrono::Utc;
use nexus_common::models::traits::Collection;
use nexus_common::models::user::{UserDetails, UserSearch};
use nexus_common::utils::test_utils::random_pubky_id;
use pubky_app_specs::PubkyAppUserLink;

/// A light Nexus keeps no names, so it has no name search.
#[tokio_shared_rt::test(shared)]
async fn test_light_name_searches_are_unavailable() -> Result<()> {
    for uri in [
        "/v0/search/users/by_name/alice",
        "/v0/stream/users/username?username=alice",
    ] {
        let (status, body) = light_request(Method::GET, uri, None).await?;
        assert_unavailable(status, &body, uri);
    }
    Ok(())
}

/// User details are served without what the user wrote: no name, bio, links or status.
/// The avatar link and the profile hash are kept.
///
/// The user is a fixture of this test's own: a light API caches what it reads without
/// those fields, so reading the shared mock data would leave it stripped for later tests.
#[tokio_shared_rt::test(shared)]
async fn test_light_user_details_carry_no_profile_fields() -> Result<()> {
    setup_light_stack().await;

    let user_id = random_pubky_id();
    let fixture = UserDetails {
        name: "Light:Users:Fixture".to_string(),
        bio: Some("bio".to_string()),
        id: user_id.clone(),
        links: Some(vec![PubkyAppUserLink {
            title: "site".to_string(),
            url: "https://example.com".to_string(),
        }]),
        status: Some("around".to_string()),
        image: Some(format!(
            "pubky://{user_id}/pub/pubky.app/files/0034AVATAR000"
        )),
        indexed_at: Utc::now().timestamp_millis(),
        deleted: false,
        profile_hash: Some("0".repeat(64)),
    };
    fixture.put_to_graph().await?;

    let (status, body) =
        light_request(Method::GET, &format!("/v0/user/{user_id}/details"), None).await?;
    // The read indexed the id for id search; that goes too.
    UserSearch::delete(&user_id).await?;
    UserDetails::delete(&user_id).await?;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(body["id"], user_id.to_string());
    for field in ["name", "bio", "links", "status"] {
        assert!(body.get(field).is_none(), "no {field}: {body}");
    }
    assert_eq!(body["image"], serde_json::json!(fixture.image));
    assert_eq!(body["profile_hash"], "0".repeat(64));
    Ok(())
}
