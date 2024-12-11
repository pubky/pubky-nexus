use crate::watcher::utils::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use pubky_app_specs::{PubkyAppFile, PubkyAppUser};
use pubky_common::crypto::Keypair;
use pubky_common::timestamp::Timestamp;
use pubky_nexus::models::{file::FileDetails, traits::Collection};
use serde_json::to_vec;

#[tokio_shared_rt::test(shared)]
async fn test_put_pubkyapp_file() -> Result<()> {
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
    test.client.put(blob_url.as_str(), &json_data).await?;

    // Act
    let file = PubkyAppFile {
        name: "myfile".to_string(),
        content_type: "text/plain".to_string(),
        src: blob_url.clone(),
        size: json_data.len() as i64,
        created_at: Utc::now().timestamp_millis(),
    };

    let (file_id, _) = test.create_file(&user_id, &file).await?;

    // Assert
    let files = FileDetails::get_by_ids(
        vec![vec![user_id.as_str(), file_id.as_str()].as_slice()].as_slice(),
    )
    .await
    .expect("Failed to fetch files from Nexus");

    let result_file = files[0].as_ref().expect("Created file was not found.");

    assert_eq!(result_file.id, file_id);
    assert_eq!(result_file.src, blob_url);
    assert_eq!(
        result_file.uri,
        format!("pubky://{user_id}/pub/pubky.app/files/{file_id}")
    );
    assert_eq!(result_file.size, file.size);
    assert_eq!(result_file.name, file.name);
    assert_eq!(result_file.owner_id, user_id);

    // Assert: Ensure it's statically served
    let nexus_url = format!(
        "http://{}:{}",
        test.config.server_host, test.config.server_port
    );
    let client = httpc_test::new_client(nexus_url)?;

    let blob_static_path = format!("/static/files/{}", result_file.urls.main.clone());

    let response = client.do_get(&blob_static_path).await?;

    assert_eq!(response.status(), 200);
    assert_eq!(
        response
            .header("content-length")
            .unwrap()
            .parse::<i32>()
            .unwrap(),
        14
    );
    assert_eq!(response.header("content-type").unwrap(), file.content_type);

    Ok(())
}
