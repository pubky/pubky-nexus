use std::panic::{resume_unwind, AssertUnwindSafe};

use anyhow::Result;
use chrono::Utc;
use futures_util::FutureExt;
use nexus_common::models::{file::FileDetails, traits::Collection};
use pubky_social_specs::legacy_v0::file_uri_builder;

use crate::utils::get_request;

// From docker/test-graph/mocks/files.cypher; no other test reads this file.
const GRAPH_UPDATE_FILE_OWNER: &str = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
const GRAPH_UPDATE_FILE_ID: &str = "2ZK4GRPHUPD00";
const GRAPH_UPDATE_FILE_NAME: &str = "graph_update_file";

fn file_details_path(uri: &str) -> String {
    format!(
        "/v0/files/file/{}",
        url::form_urlencoded::byte_serialize(uri.as_bytes()).collect::<String>()
    )
}

#[tokio_shared_rt::test(shared)]
async fn test_file_details() -> Result<()> {
    let test_file_id = "2ZK2H8P2T5NG0";
    let test_file_user = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";

    let test_file_uri = file_uri_builder(test_file_user.into(), test_file_id.into());

    let json_body = get_request(&file_details_path(&test_file_uri)).await?;

    assert_eq!(json_body["id"], test_file_id);
    assert_eq!(json_body["owner_id"], test_file_user);

    Ok(())
}

/// A change to a file's graph metadata shows on the next read, whatever Redis holds.
#[tokio_shared_rt::test(shared)]
async fn test_file_details_follow_graph_updates() -> Result<()> {
    let uri = file_uri_builder(GRAPH_UPDATE_FILE_OWNER.into(), GRAPH_UPDATE_FILE_ID.into());
    let path = file_details_path(&uri);

    let body = get_request(&path).await?;
    assert_eq!(body["name"], GRAPH_UPDATE_FILE_NAME);

    let mut file = FileDetails::get_by_ids(&[&[GRAPH_UPDATE_FILE_OWNER, GRAPH_UPDATE_FILE_ID]])
        .await?
        .pop()
        .flatten()
        .expect("fixture file in the graph");
    let renamed = format!("renamed-{}", Utc::now().timestamp_millis());
    file.name = renamed.clone();
    file.put_to_graph().await?;

    // Catch a failed request, error or status panic, so it cannot skip the restore below.
    let read = AssertUnwindSafe(get_request(&path)).catch_unwind().await;

    // Put the fixture back before checking the read, so a failure does not leave it renamed.
    file.name = GRAPH_UPDATE_FILE_NAME.to_string();
    file.put_to_graph().await?;

    let body = read.unwrap_or_else(|panic| resume_unwind(panic))?;
    assert_eq!(body["name"], renamed);

    Ok(())
}
