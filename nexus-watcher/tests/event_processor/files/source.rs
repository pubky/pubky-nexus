//! A v0 file whose `src` can never be read is dropped, not retried: a `src` that is no blob
//! address fails in translation, before any download, and a blob whose bytes don't hash to its
//! id fails in the handler. Prior art: `create.rs`, `blacklist.rs` and `oversized.rs`.

use crate::event_processor::utils::default_moderation_tests;
use crate::event_processor::utils::watcher::{retrieve_and_handle_event_line, WatcherTest};
use crate::service::utils::{new_in_memory_store, setup, MockHomeserverClient};
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::file::FileDetails;
use nexus_common::models::traits::Collection;
use nexus_common::utils::test_utils::{default_ingestor_tests, random_pubky_id};
use nexus_watcher::events::retry::event::RetryEvent;
use nexus_watcher::events::retry::{IndexKey, RetryScheduler};
use nexus_watcher::events::{DefaultEventHandler, EventHandler};
use nexus_watcher::EventProcessorError;
use pubky::Keypair;
use pubky_social_specs::legacy_v0::traits::{HasIdPath, HashId, TimestampId};
use pubky_social_specs::legacy_v0::{
    blob_uri_builder, file_uri_builder, post_uri_builder, PubkyAppBlob, PubkyAppFile, PubkyAppUser,
};
use std::path::Path;
use std::sync::Arc;

/// A v0 file's metadata pointing at `src`.
fn file_with_src(src: String) -> PubkyAppFile {
    PubkyAppFile {
        name: "myfile".to_string(),
        content_type: "text/plain".to_string(),
        src,
        size: 12,
        created_at: Utc::now().timestamp_millis(),
    }
}

/// The error is deterministic: dropped, not queued for retry.
fn assert_spec_validation_not_retried(err: &EventProcessorError) {
    assert!(
        matches!(err, EventProcessorError::SpecValidation(_)),
        "expected SpecValidation, got {err:?}"
    );
    assert_not_retried(err);
}

/// A source Nexus can never fetch: dropped as an unsupported target, not queued for retry.
fn assert_unsupported_target_not_retried(err: &EventProcessorError) {
    assert!(
        matches!(err, EventProcessorError::UnsupportedTarget(_)),
        "expected UnsupportedTarget, got {err:?}"
    );
    assert_not_retried(err);
}

fn assert_not_retried(err: &EventProcessorError) {
    assert!(
        !RetryScheduler::should_enqueue_related_event(err),
        "a deterministic rejection must not be retried, got {err:?}"
    );
}

/// Neither a `File` node nor a folder on disk exists for the file.
async fn assert_nothing_indexed(files_path: &Path, owner_id: &str, file_id: &str) -> Result<()> {
    let stored = FileDetails::get_by_ids(&[&[owner_id, file_id]]).await?;
    assert!(
        stored.is_empty() || stored[0].is_none(),
        "a dropped file must not be in the graph"
    );
    assert!(
        !files_path.join(owner_id).join(file_id).exists(),
        "a dropped file must not be written to disk"
    );
    Ok(())
}

/// Handles the PUT of `file` for `owner_id` through the real handler over a mock homeserver
/// that serves only the metadata, and returns the error with the blob URIs the handler asked
/// the homeserver for.
async fn handle_over_mock(
    owner_id: &str,
    file: &PubkyAppFile,
    files_path: &Path,
) -> Result<(EventProcessorError, Vec<String>)> {
    let file_uri = file_uri_builder(owner_id.to_string(), file.create_id());
    let client = Arc::new(
        MockHomeserverClient::default().with_resource(&file_uri, serde_json::to_vec(file)?),
    );
    let handler: Arc<dyn EventHandler> = Arc::new(DefaultEventHandler::new(
        default_moderation_tests(),
        default_ingestor_tests(),
        client.clone(),
        files_path.to_path_buf(),
        new_in_memory_store(),
    ));
    let err = retrieve_and_handle_event_line(&format!("PUT {file_uri}"), handler)
        .await
        .expect_err("a file with an unreadable source must be rejected");
    Ok((err, client.blob_fetches().await))
}

/// A `src` that is a URL but no `pubky://` address was a retried `Generic` error. It is now
/// rejected in translation as an unsupported target, and the handler never asks for a blob.
#[tokio_shared_rt::test(shared)]
async fn test_file_with_a_source_that_is_not_a_pubky_uri_is_dropped_without_a_download(
) -> Result<()> {
    setup().await?;
    let files = tempfile::TempDir::new()?;
    let owner_id = random_pubky_id().to_string();
    let file = file_with_src("https://example.com/photo.png".to_string());

    let (err, blob_fetches) = handle_over_mock(&owner_id, &file, files.path()).await?;

    assert_unsupported_target_not_retried(&err);
    assert!(
        blob_fetches.is_empty(),
        "no blob must be fetched for a source that is not a pubky URI, got {blob_fetches:?}"
    );
    assert_nothing_indexed(files.path(), &owner_id, &file.create_id()).await
}

/// A `src` that names a `pubky.app` resource other than a blob was downloaded and then
/// rejected as `InvalidEventLine`. It is now rejected in translation, before any download.
#[tokio_shared_rt::test(shared)]
async fn test_file_with_a_source_that_is_not_a_blob_address_is_dropped_without_a_download(
) -> Result<()> {
    setup().await?;
    let files = tempfile::TempDir::new()?;
    let owner_id = random_pubky_id().to_string();
    let file = file_with_src(post_uri_builder(
        owner_id.clone(),
        "0032SSN7Q4EVG".to_string(),
    ));

    let (err, blob_fetches) = handle_over_mock(&owner_id, &file, files.path()).await?;

    assert_spec_validation_not_retried(&err);
    assert!(
        blob_fetches.is_empty(),
        "no blob must be fetched for a source that is not a blob address, got {blob_fetches:?}"
    );
    assert_nothing_indexed(files.path(), &owner_id, &file.create_id()).await
}

/// A blob whose bytes don't hash to the id in its address was a retried `Generic` error. It is
/// now spec validation: the download is dropped, nothing is written, and the event is not queued
/// for retry.
#[tokio_shared_rt::test(shared)]
async fn test_file_whose_blob_does_not_match_its_id_is_dropped() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Watcher:File:BlobMismatch".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    // The homeserver does not check a blob against its id: store one set of bytes under the id
    // of another.
    let blob_id = PubkyAppBlob::new(b"Hello World!".to_vec()).create_id();
    test.create_file_from_body(
        &user_kp,
        &PubkyAppBlob::create_path(&blob_id),
        b"Goodbye World!".to_vec(),
    )
    .await?;
    let file = file_with_src(blob_uri_builder(user_id.clone(), blob_id));

    // Handle the file's PUT directly, so the error class is observable.
    test = test.remove_event_processing().await;
    let (file_id, file_path) = test.create_file(&user_kp, &file).await?;
    let file_uri = file_uri_builder(user_id.clone(), file_id.clone());
    let err = retrieve_and_handle_event_line(
        &format!("PUT {file_uri}"),
        test.event_processor_runner.event_handler.clone(),
    )
    .await
    .expect_err("a blob that does not match its id must be rejected");

    assert_spec_validation_not_retried(&err);
    assert_nothing_indexed(test.temp_dir.path(), &user_id, &file_id).await?;

    // The full processor drops it the same way: nothing indexed, nothing queued.
    test.ensure_event_processing = true;
    test.ensure_event_processing_complete().await?;
    assert_nothing_indexed(test.temp_dir.path(), &user_id, &file_id).await?;
    assert!(
        !RetryEvent::check_index_key(&IndexKey::for_uri(&file_uri)).await?,
        "a blob mismatch must not be queued for retry"
    );

    test.cleanup_file(&user_kp, &file_path).await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}
