use crate::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::{file::FileDetails, traits::Collection};
use pubky::Keypair;
use pubky_app_specs::{traits::HasPath, PubkyAppBlob, PubkyAppFile, PubkyAppUser};
use std::path::Path;

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

    let blob_data = "Hello World!".to_string();
    let blob = PubkyAppBlob::new(blob_data.as_bytes().to_vec());
    let blob_url = format!("pubky://{}{}", user_id, blob.create_path());

    test.create_file_from_body(blob_url.as_str(), blob.0.clone())
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
    let files_before_delete = FileDetails::get_by_ids(
        vec![vec![user_id.as_str(), file_id.as_str()].as_slice()].as_slice(),
    )
    .await
    .expect("Failed to fetch files from Nexus");

    let file_before_delete = files_before_delete[0].as_ref();
    assert!(file_before_delete.is_some());

    test.cleanup_file(&user_id, &file_id).await?;

    // Assert
    let files = FileDetails::get_by_ids(
        vec![vec![user_id.as_str(), file_id.as_str()].as_slice()].as_slice(),
    )
    .await
    .expect("Failed to fetch files from Nexus");

    let result_file = files[0].as_ref();
    assert!(result_file.is_none());

    // Assert: Ensure it's deleted
    let blob_static_path = format!("./static/files/{}/{}/main", &user_id, &file_id);
    assert!(
        !Path::new(&blob_static_path).exists(),
        "File cannot exist after DEL event"
    );

    Ok(())
}
