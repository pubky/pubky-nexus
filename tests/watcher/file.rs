use super::utils::WatcherTest;
use anyhow::Result;
use pkarr::Keypair;
use pubky_common::timestamp::Timestamp;
use pubky_nexus::models::{
    file::FileDetails,
    pubky_app::{PubkyAppFile, PubkyAppUser},
    traits::Collection,
};
use serde_json::to_vec;

#[tokio::test]
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
    let blob_url = format!("pubky://{}/pub/pubky-app/blobs/{}", user_id, blob_id);
    let json_data = to_vec(blob)?;
    test.client.put(blob_url.as_str(), &json_data).await?;

    // Act
    let file = PubkyAppFile {
        name: "myfile".to_string(),
        content_type: "text/plain".to_string(),
        src: blob_url.clone(),
        size: json_data.len() as u64,
    };

    let file_id = test.create_file(&user_id, &file).await?;

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
        format!("pubky://{user_id}/pub/pubky-app/files/{file_id}")
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

    let blob_path = format!("/static/files/{}/{}", user_id, file_id);
    let response = client.do_get(&blob_path).await?;

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

#[tokio::test]
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
    let blob_url = format!("pubky://{}/pub/pubky-app/blobs/{}", user_id, blob_id);
    let json_data = to_vec(blob)?;
    test.client.put(blob_url.as_str(), &json_data).await?;

    let file = PubkyAppFile {
        name: "myfile".to_string(),
        content_type: "text/plain".to_string(),
        src: blob_url.clone(),
        size: 12,
    };

    let file_id = test.create_file(&user_id, &file).await?;

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
