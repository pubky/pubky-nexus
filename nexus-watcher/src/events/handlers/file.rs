use crate::events::translation::legacy_v0::read_blob;
use crate::events::EventProcessorError;
use crate::homeserver::HomeserverClient;

use nexus_common::media::{file_folder, FileVariant};
use nexus_common::models::user::UserIngestor;
use nexus_common::models::{file::FileDetails, traits::Collection};
use pubky::PubkyResource;
use pubky_social_specs::PubkyId;
use std::path::Path;
use tokio::fs::{self, remove_dir_all};
use tracing::{debug, warn};

use crate::events::inputs::{FileInput, ResourceKey};

#[tracing::instrument(name = "file.put", skip_all, fields(user_id = %key.owner_id, file_id = %key.resource_id))]
pub async fn sync_put(
    file: FileInput,
    key: ResourceKey,
    files_path: &Path,
    ingestor: &UserIngestor,
    client: &dyn HomeserverClient,
) -> Result<(), EventProcessorError> {
    debug!("Indexing file");

    ingest(
        &key.owner_id,
        &key.resource_id,
        &file,
        files_path,
        ingestor,
        client,
    )
    .await?;

    let file_details = file.into_details(&key);
    file_details.put_to_graph().await?;

    Ok(())
}

/// Downloads the blob `file` names and writes it as the file's `main` variant.
///
/// The translator has already parsed `src` into [`FileInput::blob`], so what is left is the
/// I/O: refuse a blob hosted on a blacklisted homeserver, fetch it, check the bytes against the
/// id in the address, write them. A mismatch is spec validation and never retried.
// TODO: Move it into its own process, server, etc
#[tracing::instrument(name = "file.ingest", skip_all, fields(user_id = %user_id, file_id = %file_id))]
async fn ingest(
    user_id: &PubkyId,
    file_id: &str,
    file: &FileInput,
    files_path: &Path,
    ingestor: &UserIngestor,
    client: &dyn HomeserverClient,
) -> Result<(), EventProcessorError> {
    // Refuse to download content hosted on a blacklisted HS
    ingestor
        .ensure_hs_not_blacklisted(&file.blob.owner_id)
        .await
        .inspect_err(|e| warn!("Aborting file ingest: source {}: {e}", file.src))?;

    let source: PubkyResource = file.src.parse()?;
    let bytes = read_blob(client.get_blob(&source).await?, &file.blob.blob_id)?;

    let full_path = files_path.join(file_folder(user_id.as_ref(), file_id));
    write_main_variant(&full_path, &bytes)
        .await
        .map_err(EventProcessorError::static_save_failed)
}

/// Writes `bytes` to `<dir>/main`, creating `dir` if it does not exist.
///
/// An existing file is overwritten, which is what re-indexing an already
/// downloaded file relies on.
async fn write_main_variant(dir: &Path, bytes: &[u8]) -> std::io::Result<()> {
    fs::create_dir_all(dir).await?;

    fs::write(dir.join(FileVariant::Main.to_string()), bytes).await
}

#[tracing::instrument(name = "file.del", skip_all, fields(user_id = %key.owner_id, file_id = %key.resource_id))]
pub async fn del(key: ResourceKey, files_path: &Path) -> Result<(), EventProcessorError> {
    debug!("Deleting file");
    let result = FileDetails::get_by_ids(&[&[&key.owner_id, &key.resource_id]]).await?;

    if result.is_empty() {
        return Ok(());
    }

    let file = &result[0];
    if let Some(file_details) = file {
        file_details.delete().await?;
    }

    let full_path = files_path.join(file_folder(key.owner_id.as_ref(), &key.resource_id));

    match remove_dir_all(full_path.as_path()).await {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn read_main(dir: &Path) -> Vec<u8> {
        fs::read(dir.join("main")).await.unwrap()
    }

    #[tokio_shared_rt::test(shared)]
    async fn write_main_variant_creates_new_file() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let dir = tmp_dir.path().join("user1").join("file1");
        write_main_variant(&dir, b"hello world")
            .await
            .expect("write_main_variant should succeed for a new file");

        assert_eq!(read_main(&dir).await, b"hello world");
    }

    #[tokio_shared_rt::test(shared)]
    async fn write_main_variant_overwrites_existing_file() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let dir = tmp_dir.path().join("user1").join("file1");
        write_main_variant(&dir, b"first write")
            .await
            .expect("first write_main_variant should succeed");

        // Writing again simulates re-indexing when the file already exists on disk.
        // The second payload is shorter than the first, so a write without truncation
        // would leave stale trailing bytes and fail the assertion below.
        write_main_variant(&dir, b"second").await.expect(
            "write_main_variant should succeed even when file already exists (re-indexing)",
        );

        assert_eq!(read_main(&dir).await, b"second");
    }

    /// An I/O failure must be reported, not swallowed: the caller maps the error to
    /// `EventProcessorError::static_save_failed` so the event is retried instead of
    /// indexing URLs that point at a `main` that was never written.
    ///
    /// What fails here is opening `main` for writing. A failure part-way through the
    /// write has no portable way to provoke and is covered by construction instead:
    /// `fs::write` returns `write_all`'s error directly.
    #[tokio_shared_rt::test(shared)]
    async fn write_main_variant_propagates_io_error() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let dir = tmp_dir.path().join("user1").join("file1");
        // A directory where `main` should go: creating the parent still succeeds, but
        // `main` cannot be opened for writing, on any platform.
        fs::create_dir_all(dir.join(FileVariant::Main.to_string()))
            .await
            .unwrap();
        write_main_variant(&dir, b"hello world")
            .await
            .expect_err("write_main_variant should surface the failure to open `main`");
    }
}
