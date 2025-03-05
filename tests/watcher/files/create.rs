use crate::{
    service::utils::host_url, utils::TestServiceServer, watcher::utils::watcher::WatcherTest,
};
use anyhow::Result;
use chrono::Utc;
use pubky::Keypair;
use pubky_app_specs::traits::HasPath;
use pubky_app_specs::{PubkyAppBlob, PubkyAppFile, PubkyAppUser};
use pubky_nexus::models::{file::FileDetails, traits::Collection};

#[tokio_shared_rt::test(shared)]
async fn test_put_pubkyapp_file() -> Result<()> {
    // Arrange
    let mut test = WatcherTest::setup().await?;
    TestServiceServer::get_test_server().await;

    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Test User".to_string(),
        status: None,
    };

    let user_id = test.create_user(&keypair, &user).await?;

    let blob_data = "Hello World!".to_string();
    let blob = PubkyAppBlob::new(blob_data.as_bytes().to_vec());
    let blob_url = format!("pubky://{}{}", user_id, blob.create_path());

    test.create_file_from_body(blob_url.as_str(), blob.0.clone()).await?;

    // Act
    let file = PubkyAppFile {
        name: "myfile".to_string(),
        content_type: "text/plain".to_string(),
        src: blob_url.clone(),
        size: blob.0.len() as i64,
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
    let client = httpc_test::new_client(host_url().await)?;

    let blob_static_path = format!("/static/files/{}", result_file.urls.main.clone());

    let response = client.do_get(&blob_static_path).await?;

    assert_eq!(response.status(), 200);
    assert_eq!(
        response
            .header("content-length")
            .unwrap()
            .parse::<i32>()
            .unwrap(),
        12
    );
    assert_eq!(response.header("content-type").unwrap(), file.content_type);

    Ok(())
}
