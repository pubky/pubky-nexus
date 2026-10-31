//! The file handlers: a `pubky.app` file is a metadata object whose blob is downloaded and kept
//! with a `File` node; a `social/v1` file is raw bytes at its own address, kept on disk alone.
//! Both live in the folder [`file_folder`] picks by the id, and the one DEL handler picks the
//! same way.

use crate::events::translation::legacy_v0::read_blob;
use crate::events::EventProcessorError;
use crate::homeserver::HomeserverClient;

use nexus_common::media::{file_folder, is_v1_file_id, FileVariant, V1_FILES_DIR};
use nexus_common::models::user::UserIngestor;
use nexus_common::models::{file::FileDetails, traits::Collection};
use pubky::PubkyResource;
use pubky_social_specs::{file_uri_builder, PubkyId, PubkySocialFile};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs::{self, remove_dir_all};
use tracing::{debug, warn};

use crate::events::inputs::{FileInput, ResourceKey};

/// Where a `social/v1` file is written before it is renamed into its folder, under
/// [`V1_FILES_DIR`] so the rename stays on one filesystem. The watcher's own: not the API's
/// `.tmp` beside the variants, which it sweeps by age and treats as its own.
const V1_TEMP_DIR: &str = ".tmp";

/// Distinguishes the temp files of the loops (primary, external, retry) that may write one file
/// at once in this process; the process id separates processes sharing the folder.
static TEMP_FILE_SEQ: AtomicU64 = AtomicU64::new(0);

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

/// Stores the `social/v1` file `key` names, whose address is the hash of its bytes: downloads
/// it from its owner's homeserver, checks it against `hash` and writes it as `main`. No graph
/// write: a v1 file has no metadata and no node.
///
/// In order: a `main` already on disk ends it with no fetch, since an address names exact bytes
/// and every write checks the hash, so a replayed batch costs nothing; the owner's homeserver
/// is checked against the blacklist; the bytes are fetched under the file cap, over it being
/// [`EventProcessorError::FetchSizeExceeded`]; bytes that don't hash to the address are
/// [`EventProcessorError::SpecValidation`], neither retried; a failed write is
/// [`EventProcessorError::StaticSaveFailed`] and retried, as for a `pubky.app` file. A
/// homeserver 404 is the fetch error it is for every PUT.
#[tracing::instrument(name = "file.put_v1", skip_all, fields(user_id = %key.owner_id, file_id = %key.resource_id))]
pub async fn sync_put_v1(
    key: ResourceKey,
    hash: &str,
    files_path: &Path,
    ingestor: &UserIngestor,
    client: &dyn HomeserverClient,
) -> Result<(), EventProcessorError> {
    let folder = files_path.join(file_folder(key.owner_id.as_ref(), &key.resource_id));
    if fs::try_exists(main_path(&folder)).await? {
        debug!("File is already on disk; nothing to fetch");
        return Ok(());
    }
    debug!("Indexing v1 file");

    ingestor
        .ensure_hs_not_blacklisted(&key.owner_id)
        .await
        .inspect_err(|e| warn!("Aborting v1 file ingest: owner {}: {e}", key.owner_id))?;

    let source: PubkyResource =
        file_uri_builder(key.owner_id.to_string(), key.resource_id.clone()).parse()?;
    let bytes = client.get_blob(&source).await?;

    // Hashing up to the file cap of bytes is CPU work: off the runtime's workers.
    let hash = hash.to_string();
    let file = tokio::task::spawn_blocking(move || PubkySocialFile::from_vec(bytes, &hash))
        .await
        .map_err(EventProcessorError::internal_error)?
        .map_err(EventProcessorError::SpecValidation)?;

    write_v1_main(files_path, &folder, &file.0)
        .await
        .map_err(EventProcessorError::static_save_failed)
}

/// The `main` variant inside a file's folder.
fn main_path(folder: &Path) -> PathBuf {
    folder.join(FileVariant::Main.to_string())
}

/// Writes `bytes` as the `main` of the `social/v1` file whose folder is `folder`, under
/// `files_path`: into a temp file in `files_path/v1/.tmp` first, then a rename into place.
/// Never a direct write: a crash leaves no partial file under the final name, so an existing
/// `main` always means a complete one, and a `main` being served is replaced whole, never
/// truncated while a reader has it open. The temp file is removed when the write fails.
///
/// The flip's copy writes through here too, so both writers leave the same thing on disk.
pub(crate) async fn write_v1_main(
    files_path: &Path,
    folder: &Path,
    bytes: &[u8],
) -> std::io::Result<()> {
    let temp_dir = files_path.join(V1_FILES_DIR).join(V1_TEMP_DIR);
    fs::create_dir_all(&temp_dir).await?;
    fs::create_dir_all(folder).await?;

    let name = folder
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let temp_path = temp_dir.join(format!(
        "{name}.{}.{}",
        std::process::id(),
        TEMP_FILE_SEQ.fetch_add(1, Ordering::Relaxed)
    ));

    let written = async {
        fs::write(&temp_path, bytes).await?;
        fs::rename(&temp_path, main_path(folder)).await
    }
    .await;
    if written.is_err() {
        // Best effort: a temp file left behind holds no address, so nothing serves it.
        let _ = fs::remove_file(&temp_path).await;
    }
    written
}

/// Deletes the file `key` names, of either epoch, picking what to remove by the id. A
/// `social/v1` file has no `File` node: its folder goes, and nothing else, so a DEL of `H.png`
/// leaves `H.bin` and every legacy folder alone. A `pubky.app` file's node and folder go.
/// Deleting what isn't there is a no-op either way.
#[tracing::instrument(name = "file.del", skip_all, fields(user_id = %key.owner_id, file_id = %key.resource_id))]
pub async fn del(key: ResourceKey, files_path: &Path) -> Result<(), EventProcessorError> {
    debug!("Deleting file");
    if !is_v1_file_id(&key.resource_id) {
        let result = FileDetails::get_by_ids(&[&[&key.owner_id, &key.resource_id]]).await?;

        if result.is_empty() {
            return Ok(());
        }

        if let Some(file_details) = &result[0] {
            file_details.delete().await?;
        }
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

    // ---- the v1 writer ------------------------------------------------------------------

    /// The v1 file's folder under `files_path`, as the handler builds it.
    fn v1_folder(files_path: &Path) -> PathBuf {
        files_path.join(file_folder("owner", "8Z8CWH8NVYQY39ZEBFGKQWWEKG.png"))
    }

    /// The names left in the watcher's temp folder.
    async fn temp_files(files_path: &Path) -> Vec<String> {
        let mut names = Vec::new();
        let Ok(mut entries) = fs::read_dir(files_path.join("v1").join(".tmp")).await else {
            return names;
        };
        while let Some(entry) = entries.next_entry().await.unwrap() {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
        names
    }

    /// A write lands as `main` in the file's folder, inside `v1/`, and leaves nothing in the
    /// temp folder: the temp file is consumed by the rename.
    #[tokio_shared_rt::test(shared)]
    async fn write_v1_main_writes_main_through_the_temp_folder() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let folder = v1_folder(tmp_dir.path());

        write_v1_main(tmp_dir.path(), &folder, b"hello world")
            .await
            .expect("a first write succeeds");

        assert_eq!(read_main(&folder).await, b"hello world");
        assert!(folder.starts_with(tmp_dir.path().join("v1")));
        assert!(
            temp_files(tmp_dir.path()).await.is_empty(),
            "the temp file is consumed by the rename"
        );
    }

    /// A second write replaces `main` whole, shorter bytes included, so a reader sees the old
    /// file or the new one and never a truncated mix.
    #[tokio_shared_rt::test(shared)]
    async fn write_v1_main_replaces_an_existing_main_whole() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let folder = v1_folder(tmp_dir.path());
        write_v1_main(tmp_dir.path(), &folder, b"first write")
            .await
            .unwrap();

        write_v1_main(tmp_dir.path(), &folder, b"second")
            .await
            .expect("a rewrite succeeds");

        assert_eq!(read_main(&folder).await, b"second");
        assert!(temp_files(tmp_dir.path()).await.is_empty());
    }

    /// A failed rename reports its error and removes the temp file, so a failure leaves no
    /// partial file under the final name and nothing in the temp folder.
    #[tokio_shared_rt::test(shared)]
    async fn write_v1_main_propagates_io_error_and_leaves_no_temp_file() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let folder = v1_folder(tmp_dir.path());
        // A non-empty directory where `main` should go: the rename over it fails on any
        // platform, after the temp file was written.
        fs::create_dir_all(main_path(&folder).join("occupied"))
            .await
            .unwrap();

        write_v1_main(tmp_dir.path(), &folder, b"hello world")
            .await
            .expect_err("the failed rename is reported");

        assert!(main_path(&folder).is_dir(), "nothing was written over it");
        assert!(
            temp_files(tmp_dir.path()).await.is_empty(),
            "the temp file is removed on failure"
        );
    }
}
