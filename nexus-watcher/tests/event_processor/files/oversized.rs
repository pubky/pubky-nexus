use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::file::FileDetails;
use nexus_common::models::traits::Collection;
use pubky::Keypair;
use pubky_app_specs::traits::{HasIdPath, HashId};
use pubky_app_specs::{blob_uri_builder, PubkyAppBlob, PubkyAppFile, PubkyAppUser};

use super::super::posts::utils::find_post_details;

/// A cap small enough to reject a non-trivial blob, but large enough to avoid
/// triggering on the file metadata descriptor itself.
const TINY_MAX_FILE_SIZE: u64 = 64;

/// Creates a blob that is larger than [TINY_MAX_FILE_SIZE] bytes.
fn create_oversized_blob() -> PubkyAppBlob {
    PubkyAppBlob::new(vec![0xABu8; (TINY_MAX_FILE_SIZE + 1) as usize])
}

/// Creates a blob comfortably smaller than [TINY_MAX_FILE_SIZE] bytes.
fn create_small_blob() -> PubkyAppBlob {
    PubkyAppBlob::new(b"small file content".to_vec())
}

/// Inverse of [test_rejected_file_not_written_to_disk]: when the blob fits
/// within the cap the file is accepted — persisted in the graph and written
/// to disk.
#[tokio_shared_rt::test(shared)]
async fn test_small_file_written_to_disk() -> Result<()> {
    // Arrange: use the same tiny cap, but a blob that fits
    let mut test = WatcherTest::setup()
        .await?
        .with_max_file_size(TINY_MAX_FILE_SIZE);

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Test User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    let blob = create_small_blob();
    let blob_id = blob.create_id();
    let blob_relative_url = PubkyAppBlob::create_path(&blob_id);
    let blob_absolute_url = blob_uri_builder(user_id.clone(), blob_id);

    test.create_file_from_body(&user_kp, blob_relative_url.as_str(), blob.0.clone())
        .await?;

    let file = PubkyAppFile {
        name: "smallfile".to_string(),
        content_type: "text/plain".to_string(),
        src: blob_absolute_url.clone(),
        size: blob.0.len(),
        created_at: Utc::now().timestamp_millis(),
    };
    let (file_id, _) = test.create_file(&user_kp, &file).await?;

    // Act
    test.ensure_event_processing_complete().await?;

    // Assert: file IS in the graph
    let stored = FileDetails::get_by_ids(&[&[&user_id, &file_id]])
        .await
        .expect("Failed to fetch files from Nexus");
    assert!(
        !stored.is_empty() && stored[0].is_some(),
        "Small file must be persisted in the graph"
    );
    let file_details = stored[0].as_ref().unwrap();
    assert_eq!(file_details.name, "smallfile");

    // Assert: blob directory and file exist on disk
    let blob_dir = test.temp_dir.path().join(&user_id);
    assert!(blob_dir.exists(), "Blob directory must exist on disk");
    assert!(
        blob_dir.read_dir().unwrap().next().is_some(),
        "Blob directory must contain the downloaded file"
    );

    Ok(())
}

/// Truthful Content-Length: the homeserver advertises the real (oversized) size
/// in the Content-Length header, so the Content-Length pre-check in `ingest()`
/// rejects immediately without downloading the body.
#[tokio_shared_rt::test(shared)]
async fn test_rejected_file_not_written_to_disk() -> Result<()> {
    // Arrange: set a tiny max_file_size so the blob is oversized
    let mut test = WatcherTest::setup()
        .await?
        .with_max_file_size(TINY_MAX_FILE_SIZE);

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Test User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    // Create an oversized blob on the homeserver
    let blob = create_oversized_blob();
    let blob_id = blob.create_id();
    let blob_relative_url = PubkyAppBlob::create_path(&blob_id);
    let blob_absolute_url = blob_uri_builder(user_id.clone(), blob_id);

    test.create_file_from_body(&user_kp, blob_relative_url.as_str(), blob.0.clone())
        .await?;

    // Build the file event (metadata is small, the blob behind src is oversized)
    let file = PubkyAppFile {
        name: "bigfile".to_string(),
        content_type: "application/octet-stream".to_string(),
        src: blob_absolute_url.clone(),
        size: blob.0.len(),
        created_at: Utc::now().timestamp_millis(),
    };
    let (file_id, _) = test.create_file(&user_kp, &file).await?;

    // Act: run the event processor — the oversized file should be rejected
    test.ensure_event_processing_complete().await?;

    // Assert: file must NOT be in the graph
    let stored = FileDetails::get_by_ids(&[&[&user_id, &file_id]])
        .await
        .expect("Failed to fetch files from Nexus");
    assert!(
        stored.is_empty() || stored[0].is_none(),
        "Rejected file must not be persisted in the graph"
    );

    // Assert: nothing written to the temp dir for this file
    let expected_dir = test.temp_dir.path().join(&user_id);
    assert!(
        !expected_dir.exists(),
        "Rejected file must not create any directory on disk"
    );

    Ok(())
}

/// Creates an oversized file event followed by a valid post event in the same
/// batch, then verifies that the processor rejects the file but still indexes
/// the post — proving that rejection is non-fatal and ingestion continues.
#[tokio_shared_rt::test(shared)]
async fn test_ingestion_continues_after_rejection() -> Result<()> {
    // Arrange: set a tiny max_file_size so the blob is oversized
    let mut test = WatcherTest::setup()
        .await?
        .with_max_file_size(TINY_MAX_FILE_SIZE);

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Test User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    // 1) Create an oversized file event
    let blob = create_oversized_blob();
    let blob_id = blob.create_id();
    let blob_relative_url = PubkyAppBlob::create_path(&blob_id);
    let blob_absolute_url = blob_uri_builder(user_id.clone(), blob_id);

    test.create_file_from_body(&user_kp, blob_relative_url.as_str(), blob.0.clone())
        .await?;

    let file = PubkyAppFile {
        name: "bigfile".to_string(),
        content_type: "application/octet-stream".to_string(),
        src: blob_absolute_url.clone(),
        size: blob.0.len(),
        created_at: Utc::now().timestamp_millis(),
    };
    let (file_id, _) = test.create_file(&user_kp, &file).await?;

    // 2) Create a valid post event
    let post = pubky_app_specs::PubkyAppPost {
        content: "after rejection".to_string(),
        kind: pubky_app_specs::PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let (post_id, _) = test.create_post(&user_kp, &post).await?;

    // Act: run the event processor
    test.ensure_event_processing_complete().await?;

    // Assert: file must NOT be in the graph (rejected)
    let stored = FileDetails::get_by_ids(&[&[&user_id, &file_id]])
        .await
        .expect("Failed to fetch files from Nexus");
    assert!(
        stored.is_empty() || stored[0].is_none(),
        "Rejected file must not be persisted in the graph"
    );

    // Assert: post MUST be in the graph (ingestion continued)
    let post_details = find_post_details(&user_id, &post_id)
        .await
        .expect("Post should have been indexed after rejected file");
    assert_eq!(post_details.content, "after rejection");

    Ok(())
}
