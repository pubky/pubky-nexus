use super::{assert_unavailable, light_request, setup_light_stack};
use anyhow::Result;
use axum::http::{Method, StatusCode};
use nexus_common::models::file::FileDetails;
use nexus_common::models::traits::Collection;
use nexus_common::utils::test_utils::random_pubky_id;
use pubky_app_specs::file_uri_builder;
use serde_json::json;

// A file from the mock graph.
const FILE_ID: &str = "2ZK2H8P2T5NG0";
const FILE_OWNER: &str = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";

/// A light Nexus stores no file bytes, so none of the static routes serve.
#[tokio_shared_rt::test(shared)]
async fn test_light_static_routes_are_unavailable() -> Result<()> {
    for uri in [
        format!("/static/files/{FILE_OWNER}/{FILE_ID}/main"),
        format!("/static/files/{FILE_OWNER}/{FILE_ID}"),
        format!("/static/avatar/{FILE_OWNER}"),
    ] {
        let (status, body) = light_request(Method::GET, &uri, None).await?;
        assert_unavailable(status, &body, &uri);
    }
    Ok(())
}

/// File records are served without what a light Nexus does not keep: the name and the
/// variant URLs. `src`, type and size tell a client where and what to fetch.
///
/// The record is a fixture of this test's own: a light API caches what it reads without
/// the name, so reading the shared mock data would leave it stripped for later tests.
#[tokio_shared_rt::test(shared)]
async fn test_light_file_records_are_slim() -> Result<()> {
    // The fixture is written by a light stack, as a light watcher would.
    setup_light_stack().await;

    let owner_id = random_pubky_id().to_string();
    let file_id = "0034LIGHTFILE";
    let fixture = FileDetails {
        id: file_id.to_string(),
        uri: file_uri_builder(owner_id.clone(), file_id.into()),
        owner_id: owner_id.clone(),
        src: format!("pubky://{owner_id}/pub/pubky.app/blobs/0034LIGHTBLOB"),
        name: "holiday.png".to_string(),
        size: 42,
        content_type: "image/png".to_string(),
        ..FileDetails::default()
    };
    fixture.put_to_graph().await?;

    let (status, body) = light_request(
        Method::POST,
        "/v0/files/by_ids",
        Some(json!({ "uris": [fixture.uri.clone()] })),
    )
    .await?;
    fixture.delete().await?;
    assert_eq!(status, StatusCode::OK);

    let file = &body[0];
    assert_eq!(file["id"], file_id);
    assert_eq!(file["src"], fixture.src);
    assert_eq!(file["content_type"], "image/png");
    assert_eq!(file["size"], 42);
    assert_eq!(file["blocked"], false);
    assert!(file.get("name").is_none(), "no name: {file}");
    assert!(file.get("urls").is_none(), "no variant URLs: {file}");
    Ok(())
}
