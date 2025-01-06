use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use pubky_app_specs::{PubkyAppFile, PubkyAppUser};
use pubky_common::crypto::Keypair;
use pubky_common::timestamp::Timestamp;
use pubky_nexus::{
    models::{file::FileDetails, traits::Collection},
    PubkyConnector,
};
use serde_json::to_vec;

#[tokio_shared_rt::test(shared)]
async fn test_delete_pubkyapp_file() -> Result<()> {
    // Arrange
    let mut test = WatcherTest::setup().await?;

    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Test User".to_string(),
        status: None,
    };

    let user_id = test.create_user(&keypair, &user).await?;

    let blob = "Hello World!";
    let blob_id = Timestamp::now().to_string();
    let blob_url = format!("pubky://{}/pub/pubky.app/blobs/{}", user_id, blob_id);
    let json_data = to_vec(blob)?;
    let pubky_client = PubkyConnector::get_pubky_client()?;
    pubky_client
        .put(blob_url.as_str())
        .json(&json_data)
        .send()
        .await?;

    let file = PubkyAppFile {
        name: "myfile".to_string(),
        content_type: "text/plain".to_string(),
        src: blob_url.clone(),
        size: 12,
        created_at: Utc::now().timestamp_millis(),
    };

    let (file_id, _) = test.create_file(&user_id, &file).await?;

    // Act

    test.cleanup_file(&user_id, &file_id).await?;

    // Assert
    let files = FileDetails::get_by_ids(
        vec![vec![user_id.as_str(), file_id.as_str()].as_slice()].as_slice(),
    )
    .await
    .expect("Failed to fetch files from Nexus");

    let result_file = files[0].as_ref();

    assert!(result_file.is_none());

    // Assert: Ensure it's not served anymore
    let nexus_url = format!(
        "http://{}:{}",
        test.config.server_host, test.config.server_port
    );
    let client = httpc_test::new_client(nexus_url)?;

    let blob_path = format!("/static/files/{}/{}", user_id, file_id);
    let response = client.do_get(&blob_path).await?;

    assert_eq!(response.status(), 404);

    Ok(())
}
