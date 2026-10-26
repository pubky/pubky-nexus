use crate::events::EventProcessorError;
use crate::homeserver::HomeserverClient;

use nexus_common::media::FileVariant;
use nexus_common::models::user::UserIngestor;
use nexus_common::models::{file::FileDetails, traits::Collection};
use pubky::PubkyResource;
use pubky_app_specs::{ParsedUri, PubkyAppBlob, PubkyAppFile, PubkyAppObject, PubkyId};
use std::path::Path;
use tokio::fs::{self, remove_dir_all};
use tracing::{debug, warn};

/// Where a file event came from: the resource URI and, when known, the
/// homeserver whose stream delivered it.
pub struct FileOrigin<'a> {
    pub uri: String,
    pub homeserver: Option<&'a PubkyId>,
}

#[tracing::instrument(name = "file.put", skip_all, fields(user_id = %user_id, file_id = %file_id))]
pub async fn sync_put(
    file: PubkyAppFile,
    origin: FileOrigin<'_>,
    user_id: PubkyId,
    file_id: String,
    files_path: &Path,
    ingestor: &UserIngestor,
    client: &dyn HomeserverClient,
) -> Result<(), EventProcessorError> {
    debug!("Indexing file");

    ingest(
        &user_id,
        file_id.as_str(),
        &file,
        files_path,
        ingestor,
        client,
        origin.homeserver,
    )
    .await?;

    let file_details =
        FileDetails::from_homeserver(&file, origin.uri, user_id.to_string(), file_id);
    file_details.put_to_graph().await?;

    Ok(())
}

// TODO: Move it into its own process, server, etc
#[tracing::instrument(name = "file.ingest", skip_all, fields(user_id = %user_id, file_id = %file_id))]
async fn ingest(
    user_id: &PubkyId,
    file_id: &str,
    pubkyapp_file: &PubkyAppFile,
    files_path: &Path,
    ingestor: &UserIngestor,
    client: &dyn HomeserverClient,
    homeserver: Option<&PubkyId>,
) -> Result<(), EventProcessorError> {
    let blob = download_blob(&pubkyapp_file.src, user_id, homeserver, ingestor, client).await?;

    let dir = files_path.join(Path::new(&user_id.to_string()).join(file_id));
    write_main_variant(&dir, blob)
        .await
        .map_err(EventProcessorError::static_save_failed)
}

/// Downloads the blob a file points at. `user_id`'s own blob comes from
/// `homeserver`, the one that delivered the file event; a source on another
/// user's homeserver is fetched by URI. A source on a blacklisted homeserver
/// is refused before anything is fetched.
async fn download_blob(
    file_src: &str,
    user_id: &PubkyId,
    homeserver: Option<&PubkyId>,
    ingestor: &UserIngestor,
    client: &dyn HomeserverClient,
) -> Result<PubkyAppBlob, EventProcessorError> {
    let parsed_source_uri = ParsedUri::try_from(file_src.to_string()).map_err(|e| {
        EventProcessorError::generic(format!("Invalid file source URI {file_src}: {e}"))
    })?;

    ingestor
        .ensure_hs_not_blacklisted(&parsed_source_uri.user_id)
        .await
        .inspect_err(|e| warn!("Aborting file ingest: source {file_src}: {e}"))?;

    let source: PubkyResource = file_src.parse()?;
    let own_blob = parsed_source_uri.user_id == *user_id;
    let bytes = client
        .get_blob(&source, homeserver.filter(|_| own_blob))
        .await?;

    match PubkyAppObject::from_resource(&parsed_source_uri.resource, &bytes)
        .map_err(EventProcessorError::generic)?
    {
        PubkyAppObject::Blob(blob) => Ok(blob),
        _ => Err(EventProcessorError::InvalidEventLine(format!(
            "The file has a source uri that is not a blob path: {file_src}"
        ))),
    }
}

/// Writes `blob` to `<dir>/main`, creating `dir` if it does not exist.
///
/// An existing file is overwritten, which is what re-indexing an already
/// downloaded file relies on.
async fn write_main_variant(dir: &Path, blob: PubkyAppBlob) -> std::io::Result<()> {
    fs::create_dir_all(dir).await?;

    fs::write(dir.join(FileVariant::Main.to_string()), blob.0).await
}

#[tracing::instrument(name = "file.del", skip_all, fields(user_id = %user_id, file_id = %file_id))]
pub async fn del(
    user_id: &PubkyId,
    file_id: String,
    files_path: &Path,
) -> Result<(), EventProcessorError> {
    debug!("Deleting file");
    let result = FileDetails::get_by_ids(&[&[user_id, &file_id]]).await?;

    if result.is_empty() {
        return Ok(());
    }

    let file = &result[0];
    if let Some(file_details) = file {
        file_details.delete().await?;
    }

    let folder_path = Path::new(&user_id.to_string()).join(&file_id);
    let full_path = files_path.join(folder_path);

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
        let blob = PubkyAppBlob::new(b"hello world".to_vec());

        write_main_variant(&dir, blob)
            .await
            .expect("write_main_variant should succeed for a new file");

        assert_eq!(read_main(&dir).await, b"hello world");
    }

    #[tokio_shared_rt::test(shared)]
    async fn write_main_variant_overwrites_existing_file() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let dir = tmp_dir.path().join("user1").join("file1");
        let blob1 = PubkyAppBlob::new(b"first write".to_vec());

        write_main_variant(&dir, blob1)
            .await
            .expect("first write_main_variant should succeed");

        // Writing again simulates re-indexing when the file already exists on disk.
        // The second payload is shorter than the first, so a write without truncation
        // would leave stale trailing bytes and fail the assertion below.
        let blob2 = PubkyAppBlob::new(b"second".to_vec());
        write_main_variant(&dir, blob2).await.expect(
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
        let blob = PubkyAppBlob::new(b"hello world".to_vec());

        write_main_variant(&dir, blob)
            .await
            .expect_err("write_main_variant should surface the failure to open `main`");
    }
}
