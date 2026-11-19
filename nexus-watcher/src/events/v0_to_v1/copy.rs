//! The copy at the flip: a migrated user's files are at their `social/v1` addresses before any
//! post names them. For each blob in the [`FileMap`], one at a time, the first legacy `main` of
//! the group whose bytes hash to the blob is written as `v1/<owner>/{hash}.{ext}/main`, through
//! the file handler's temp-then-rename writer. The legacy folders stay untouched. Only `main` is
//! copied; the API derives variants on first request.
//!
//! The flip reads the graph, Redis and the legacy folders only: no homeserver fetch, so no fetch
//! error can decide a flip. A blob with no legacy `main` that can be read and passes its hash is
//! skipped with an `error!` naming the user and the address, and the flip goes on; the address
//! stays a 404 until a later read of its PUT event or a new upload. A legacy `main` that cannot
//! be read counts as missing: one bad folder must not hold the user at `v0`. A copy that cannot
//! be checked for, written or removed is [`EventProcessorError::MigrationDiskFailed`]: the copy
//! step fails as a
//! [`EventProcessorError::FlipFailed`], retried without limit while the user stays at `v0`. The
//! retry runs the copy again, and it skips what already exists.
//!
//! A blob is taken down when its key is ([`blob_takedown_key`]), or the `pubky.app` address of
//! any file that names it: a takedown recorded under a file's v0 address holds against its copy
//! too, whichever of the two the moderator's tag and the flip reach first.

use super::files::{Blob, FileMap};
use crate::events::handlers::file::{blob_takedown_key, main_path, write_v1_main};
use crate::events::translation::social_v1::read_v1_file;
use crate::events::EventProcessorError;
use nexus_common::media::file_folder;
use nexus_common::models::takedown::is_any_taken_down;
use pubky_social_specs::legacy_v0::file_uri_builder as v0_file_uri_builder;
use pubky_social_specs::{file_uri_builder, PubkyId};
use std::future::Future;
use std::io::ErrorKind;
use std::path::Path;
use tokio::fs;
use tracing::{debug, error, info, warn};

/// Copies every blob of `files` to its `social/v1` address under `files_path`, skipping the
/// ones taken down and the ones already there.
pub async fn copy(
    owner: &PubkyId,
    files: &FileMap,
    files_path: &Path,
) -> Result<(), EventProcessorError> {
    copy_with(owner, files, files_path, |uris| async move {
        Ok(is_any_taken_down(&uris).await?)
    })
    .await
}

/// [`copy`], asking `taken_down` whether any of a blob's addresses is taken down: the graph in
/// production, anything in a test, so the copy's disk behaviour is tested with no graph.
pub(super) async fn copy_with<F, Fut>(
    owner: &PubkyId,
    files: &FileMap,
    files_path: &Path,
    taken_down: F,
) -> Result<(), EventProcessorError>
where
    F: Fn(Vec<String>) -> Fut,
    Fut: Future<Output = Result<bool, EventProcessorError>>,
{
    for blob in files.blobs() {
        copy_blob(owner, &blob, files_path, &taken_down).await?;
    }
    Ok(())
}

/// Copies one blob, in order: a taken-down blob ([`takedown_addresses`]) is skipped, and a copy
/// an earlier attempt left behind is removed; an existing target is skipped, since an address
/// names exact bytes and every write checked them; the source is the first legacy `main` of the
/// group that passes the hash, none being a skip; the write goes through the temp-then-rename
/// writer; a takedown that landed during the copy removes it again.
async fn copy_blob<F, Fut>(
    owner: &PubkyId,
    blob: &Blob,
    files_path: &Path,
    taken_down: &F,
) -> Result<(), EventProcessorError>
where
    F: Fn(Vec<String>) -> Fut,
    Fut: Future<Output = Result<bool, EventProcessorError>>,
{
    let file_id = blob.file_id();
    let folder = files_path.join(file_folder(owner.as_ref(), &file_id));
    let uri = file_uri_builder(owner.to_string(), file_id);
    let addresses = takedown_addresses(owner, blob);

    if taken_down(addresses.clone()).await? {
        // A copy left behind by an earlier attempt whose removal failed goes here, as a PUT's
        // first takedown check removes one.
        remove_folder(&folder).await?;
        debug!(user_id = %owner, address = %uri, "File is taken down; not copied");
        return Ok(());
    }
    if fs::try_exists(main_path(&folder))
        .await
        .map_err(EventProcessorError::migration_disk_failed)?
    {
        debug!(user_id = %owner, address = %uri, "File is already on disk; not copied again");
        return Ok(());
    }

    let Some(bytes) = read_source(owner, blob, files_path).await? else {
        error!(
            user_id = %owner,
            address = %uri,
            "Migration skipped a file: no legacy main holds the bytes its address names"
        );
        return Ok(());
    };
    write_v1_main(files_path, &folder, &bytes)
        .await
        .map_err(EventProcessorError::migration_disk_failed)?;

    if taken_down(addresses).await? {
        remove_folder(&folder).await?;
        info!(user_id = %owner, address = %uri, "File was taken down during its copy; removed");
    }
    Ok(())
}

/// The addresses a takedown of `blob` is recorded under: its key and the `pubky.app` address of
/// every file of the group. One taken-down file takes down the blob: the bytes are the content
/// the moderator removed, whichever file names them.
fn takedown_addresses(owner: &PubkyId, blob: &Blob) -> Vec<String> {
    std::iter::once(blob_takedown_key(owner.as_ref(), &blob.hash))
        .chain(
            blob.tsids
                .iter()
                .map(|tsid| v0_file_uri_builder(owner.to_string(), tsid.clone())),
        )
        .collect()
}

/// The bytes of the first legacy `main` of `blob`'s group that hashes to the blob, lowest
/// `tsid` first; `None` when no file of the group has one. A missing `main` is skipped, a
/// `main` that cannot be read is skipped with an error, and a `main` with other bytes is
/// skipped with a warning: none of them is a reason to hold the flip.
async fn read_source(
    owner: &PubkyId,
    blob: &Blob,
    files_path: &Path,
) -> Result<Option<Vec<u8>>, EventProcessorError> {
    for tsid in &blob.tsids {
        let legacy_main = main_path(&files_path.join(file_folder(owner.as_ref(), tsid)));
        let bytes = match fs::read(&legacy_main).await {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                debug!(user_id = %owner, file_id = %tsid, "Legacy main is not on disk");
                continue;
            }
            Err(e) => {
                error!(user_id = %owner, file_id = %tsid, error = %e, "Legacy main cannot be read; trying the next file of the blob");
                continue;
            }
        };
        match read_v1_file(bytes, &blob.hash).await {
            Ok(bytes) => return Ok(Some(bytes)),
            Err(EventProcessorError::SpecValidation(reason)) => {
                warn!(user_id = %owner, file_id = %tsid, hash = %blob.hash, %reason, "Legacy main does not hash to its blob");
            }
            Err(e) => return Err(e),
        }
    }
    Ok(None)
}

/// Removes a `social/v1` file's folder; one that is not there is nothing to remove.
async fn remove_folder(folder: &Path) -> Result<(), EventProcessorError> {
    match fs::remove_dir_all(folder).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(EventProcessorError::migration_disk_failed(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::handlers::file::temp_files;
    use pubky_social_specs::legacy_v0::blob_uri_builder;
    use pubky_social_specs::traits::HashId;
    use pubky_social_specs::PubkySocialFile;
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use tempfile::TempDir;

    const OWNER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";

    fn owner() -> PubkyId {
        PubkyId::try_from(OWNER).unwrap()
    }

    /// A v0 file of the owner on disk and in the map: its `tsid`, the blob its `src` names and
    /// its content type. `on_disk` is what its legacy `main` holds: the blob's bytes, other
    /// bytes, or nothing.
    struct LegacyFile {
        tsid: &'static str,
        blob: Vec<u8>,
        content_type: &'static str,
        on_disk: Option<Vec<u8>>,
    }

    impl LegacyFile {
        fn new(tsid: &'static str, blob: &[u8], content_type: &'static str) -> Self {
            Self {
                tsid,
                blob: blob.to_vec(),
                content_type,
                on_disk: Some(blob.to_vec()),
            }
        }

        fn holding(mut self, bytes: &[u8]) -> Self {
            self.on_disk = Some(bytes.to_vec());
            self
        }

        fn missing(mut self) -> Self {
            self.on_disk = None;
            self
        }

        fn hash(&self) -> String {
            PubkySocialFile(self.blob.clone()).create_id()
        }

        fn row(&self) -> (String, String, String) {
            (
                self.tsid.to_string(),
                blob_uri_builder(OWNER.to_string(), self.hash()),
                self.content_type.to_string(),
            )
        }

        fn legacy_main(&self, files_path: &Path) -> PathBuf {
            main_path(&files_path.join(file_folder(OWNER, self.tsid)))
        }
    }

    /// The owner's files, written to `files_path` as the watcher left them.
    async fn seed(files_path: &Path, files: &[LegacyFile]) -> FileMap {
        for file in files {
            if let Some(bytes) = &file.on_disk {
                let main = file.legacy_main(files_path);
                fs::create_dir_all(main.parent().unwrap()).await.unwrap();
                fs::write(&main, bytes).await.unwrap();
            }
        }
        FileMap::from_files(&owner(), files.iter().map(LegacyFile::row))
    }

    fn v1_uri(hash: &str, ext: &str) -> String {
        file_uri_builder(OWNER.to_string(), format!("{hash}.{ext}"))
    }

    /// The key a takedown of the owner's blob `hash` is recorded under.
    fn blob_key(hash: &str) -> String {
        blob_takedown_key(OWNER, hash)
    }

    fn v1_folder(files_path: &Path, hash: &str, ext: &str) -> PathBuf {
        files_path.join(file_folder(OWNER, &format!("{hash}.{ext}")))
    }

    async fn v1_main(files_path: &Path, hash: &str, ext: &str) -> Option<Vec<u8>> {
        fs::read(main_path(&v1_folder(files_path, hash, ext)))
            .await
            .ok()
    }

    /// No address is taken down.
    async fn nothing_taken_down(_uris: Vec<String>) -> Result<bool, EventProcessorError> {
        Ok(false)
    }

    fn v0_uri(tsid: &str) -> String {
        v0_file_uri_builder(OWNER.to_string(), tsid.to_string())
    }

    /// Every legacy `main` the test wrote is still there, with its bytes.
    async fn assert_legacy_intact(files_path: &Path, files: &[LegacyFile]) {
        for file in files {
            if let Some(bytes) = &file.on_disk {
                assert_eq!(
                    fs::read(file.legacy_main(files_path)).await.unwrap(),
                    *bytes,
                    "legacy main of {} must be left in place",
                    file.tsid
                );
            }
        }
    }

    /// Each blob is copied once, under its namer's extension, from a legacy `main` that is left
    /// in place; two uploads of one blob give one copy, and no temp file is left behind.
    #[tokio::test]
    async fn copies_each_blob_once_and_leaves_the_legacy_folders_in_place() {
        let dir = TempDir::new().unwrap();
        let files = [
            LegacyFile::new("0032SSN7Q4EVG", b"copy:shared", "image/png"),
            LegacyFile::new("0032SSN7Q4EVH", b"copy:shared", "text/plain"),
            LegacyFile::new("0032SSN7Q4EVJ", b"copy:alone", "application/octet-stream"),
        ];
        let map = seed(dir.path(), &files).await;

        copy_with(&owner(), &map, dir.path(), nothing_taken_down)
            .await
            .unwrap();

        let shared = files[0].hash();
        let alone = files[2].hash();
        assert_eq!(
            v1_main(dir.path(), &shared, "png").await.as_deref(),
            Some(b"copy:shared".as_slice())
        );
        assert!(
            !v1_folder(dir.path(), &shared, "txt").exists(),
            "one blob gives one copy, under the lowest tsid's extension"
        );
        assert_eq!(
            v1_main(dir.path(), &alone, "bin").await.as_deref(),
            Some(b"copy:alone".as_slice())
        );
        assert_legacy_intact(dir.path(), &files).await;
        assert!(temp_files(dir.path()).await.is_empty());
    }

    /// A blob whose only legacy `main` holds other bytes, and one whose `main` is gone, are
    /// skipped with an error naming the user and the address; the rest are copied and the step
    /// passes.
    #[tokio::test]
    #[tracing_test::traced_test]
    async fn a_bad_and_a_missing_source_are_skipped_with_an_error_and_the_rest_copied() {
        let dir = TempDir::new().unwrap();
        let files = [
            LegacyFile::new("0032SSN7Q4EVG", b"copy:good", "image/png"),
            LegacyFile::new("0032SSN7Q4EVH", b"copy:bad", "image/png").holding(b"copy:other"),
            LegacyFile::new("0032SSN7Q4EVJ", b"copy:missing", "image/png").missing(),
        ];
        let map = seed(dir.path(), &files).await;

        copy_with(&owner(), &map, dir.path(), nothing_taken_down)
            .await
            .expect("a permanently bad file must not fail the flip");

        assert_eq!(
            v1_main(dir.path(), &files[0].hash(), "png")
                .await
                .as_deref(),
            Some(b"copy:good".as_slice())
        );
        for file in &files[1..] {
            assert!(
                !v1_folder(dir.path(), &file.hash(), "png").exists(),
                "{} must not be copied",
                file.tsid
            );
            assert!(logs_contain(&v1_uri(&file.hash(), "png")), "{}", file.tsid);
        }
        assert!(logs_contain("Migration skipped a file"));
        assert!(logs_contain(&format!("user_id={OWNER}")));
        assert_legacy_intact(dir.path(), &files).await;
    }

    /// A group whose first `tsid` holds other bytes is copied from a later one that passes,
    /// still under the first one's extension.
    #[tokio::test]
    async fn a_group_whose_first_tsid_is_bad_is_copied_from_a_later_one() {
        let dir = TempDir::new().unwrap();
        let files = [
            LegacyFile::new("0032SSN7Q4EVG", b"copy:group", "image/png").holding(b"copy:other"),
            LegacyFile::new("0032SSN7Q4EVH", b"copy:group", "image/png").missing(),
            LegacyFile::new("0032SSN7Q4EVJ", b"copy:group", "text/plain"),
        ];
        let map = seed(dir.path(), &files).await;

        copy_with(&owner(), &map, dir.path(), nothing_taken_down)
            .await
            .unwrap();

        assert_eq!(
            v1_main(dir.path(), &files[0].hash(), "png")
                .await
                .as_deref(),
            Some(b"copy:group".as_slice())
        );
        assert!(!v1_folder(dir.path(), &files[0].hash(), "txt").exists());
    }

    /// A target already on disk is left as it is: a rerun rewrites nothing.
    #[tokio::test]
    async fn an_existing_target_is_not_rewritten() {
        let dir = TempDir::new().unwrap();
        let files = [LegacyFile::new(
            "0032SSN7Q4EVG",
            b"copy:exists",
            "image/png",
        )];
        let map = seed(dir.path(), &files).await;
        let hash = files[0].hash();
        let folder = v1_folder(dir.path(), &hash, "png");
        fs::create_dir_all(&folder).await.unwrap();
        fs::write(main_path(&folder), b"sentinel").await.unwrap();

        copy_with(&owner(), &map, dir.path(), nothing_taken_down)
            .await
            .unwrap();

        assert_eq!(
            v1_main(dir.path(), &hash, "png").await.as_deref(),
            Some(b"sentinel".as_slice())
        );
    }

    /// Nothing in the map: nothing is written, and the owner gets no `v1` folder.
    #[tokio::test]
    async fn nothing_to_copy_writes_nothing() {
        let dir = TempDir::new().unwrap();

        copy_with(
            &owner(),
            &FileMap::default(),
            dir.path(),
            nothing_taken_down,
        )
        .await
        .unwrap();

        assert!(!dir.path().join("v1").exists());
    }

    /// A taken-down blob is not copied, and a copy an earlier attempt left behind is removed.
    /// The key carries no extension, so the `text/plain` file now naming the blob, and the
    /// `H.txt` the copy would land at, change nothing.
    #[tokio::test]
    async fn a_taken_down_blob_is_not_copied_and_a_leftover_copy_is_removed() {
        let dir = TempDir::new().unwrap();
        let files = [
            LegacyFile::new("0032SSN7Q4EVH", b"copy:down", "text/plain"),
            LegacyFile::new("0032SSN7Q4EVJ", b"copy:up", "image/png"),
        ];
        let map = seed(dir.path(), &files).await;
        let down = files[0].hash();
        let leftover = v1_folder(dir.path(), &down, "txt");
        fs::create_dir_all(&leftover).await.unwrap();
        fs::write(main_path(&leftover), b"copy:down").await.unwrap();
        let down_key = blob_key(&down);

        copy_with(&owner(), &map, dir.path(), |uris| {
            let down = uris.contains(&down_key);
            async move { Ok(down) }
        })
        .await
        .unwrap();

        assert!(!leftover.exists(), "the leftover copy must be removed");
        assert_eq!(
            v1_main(dir.path(), &files[1].hash(), "png")
                .await
                .as_deref(),
            Some(b"copy:up".as_slice())
        );
        assert_legacy_intact(dir.path(), &files).await;
    }

    /// A takedown that lands while the file is being copied holds: the second check removes
    /// the copy, and the legacy folder stays.
    #[tokio::test]
    async fn a_takedown_landing_during_the_copy_removes_it() {
        let dir = TempDir::new().unwrap();
        let files = [LegacyFile::new("0032SSN7Q4EVG", b"copy:raced", "image/png")];
        let map = seed(dir.path(), &files).await;
        let hash = files[0].hash();
        // Down from the second check on: the moderator's tag landed during the copy.
        let asked = Mutex::new(HashSet::new());

        copy_with(&owner(), &map, dir.path(), |uris| {
            let first = asked.lock().unwrap().insert(uris);
            async move { Ok(!first) }
        })
        .await
        .unwrap();

        assert!(
            !v1_folder(dir.path(), &hash, "png").exists(),
            "the copy must be removed"
        );
        assert_legacy_intact(dir.path(), &files).await;
        assert!(temp_files(dir.path()).await.is_empty());
    }

    /// A write that fails is the migration's disk error, which never stops a batch; it leaves
    /// no file under the final name. The disk repaired, the retry copies what is missing and rewrites
    /// nothing that exists.
    #[tokio::test]
    async fn a_failed_write_is_a_migration_disk_error_and_leaves_no_file_under_the_final_name() {
        let dir = TempDir::new().unwrap();
        let files = [
            LegacyFile::new("0032SSN7Q4EVG", b"copy:write:a", "image/png"),
            LegacyFile::new("0032SSN7Q4EVH", b"copy:write:b", "image/png"),
        ];
        let map = seed(dir.path(), &files).await;
        let (a, b) = (files[0].hash(), files[1].hash());
        // A file where the writer's temp folder goes: no write can start, as on a full disk.
        let blocked = dir.path().join("v1").join(".tmp");
        fs::create_dir_all(dir.path().join("v1")).await.unwrap();
        fs::write(&blocked, b"").await.unwrap();

        let err = copy_with(&owner(), &map, dir.path(), nothing_taken_down)
            .await
            .expect_err("a failed write must fail the flip");

        assert!(
            matches!(err, EventProcessorError::MigrationDiskFailed(_)),
            "{err:?}"
        );
        assert!(!err.should_not_retry_now());
        for hash in [&a, &b] {
            assert!(
                !v1_folder(dir.path(), hash, "png").exists(),
                "nothing was written under the final name"
            );
        }
        assert_legacy_intact(dir.path(), &files).await;

        // The disk repaired, with one copy already there, the retry copies the other and
        // rewrites nothing.
        fs::remove_file(&blocked).await.unwrap();
        let existing = v1_folder(dir.path(), &a, "png");
        fs::create_dir_all(&existing).await.unwrap();
        fs::write(main_path(&existing), b"sentinel").await.unwrap();
        copy_with(&owner(), &map, dir.path(), nothing_taken_down)
            .await
            .unwrap();
        assert_eq!(
            v1_main(dir.path(), &a, "png").await.as_deref(),
            Some(b"sentinel".as_slice())
        );
        assert_eq!(
            v1_main(dir.path(), &b, "png").await.as_deref(),
            Some(files[1].blob.as_slice())
        );
        assert!(temp_files(dir.path()).await.is_empty());
    }

    /// A legacy `main` that cannot be read, other than by not being there, is skipped with an
    /// error naming the file, as a missing one is: the group's next file is the source, and a
    /// blob with no readable source is not copied. The flip passes, so one bad folder cannot
    /// hold the user at `v0`.
    #[tokio::test]
    #[tracing_test::traced_test]
    async fn an_unreadable_legacy_main_is_skipped_and_the_next_file_is_the_source() {
        let dir = TempDir::new().unwrap();
        let files = [
            LegacyFile::new("0032SSN7Q4EVG", b"copy:unreadable", "image/png").missing(),
            LegacyFile::new("0032SSN7Q4EVH", b"copy:unreadable", "image/png"),
            LegacyFile::new("0032SSN7Q4EVJ", b"copy:unreadable:alone", "image/png").missing(),
        ];
        let map = seed(dir.path(), &files).await;
        // A directory where a legacy `main` should be: reading it fails, and not with NotFound.
        for file in [&files[0], &files[2]] {
            fs::create_dir_all(file.legacy_main(dir.path()))
                .await
                .unwrap();
        }

        copy_with(&owner(), &map, dir.path(), nothing_taken_down)
            .await
            .expect("an unreadable legacy main must not fail the flip");

        assert_eq!(
            v1_main(dir.path(), &files[0].hash(), "png")
                .await
                .as_deref(),
            Some(b"copy:unreadable".as_slice()),
            "the group's readable file is the source"
        );
        assert!(!v1_folder(dir.path(), &files[2].hash(), "png").exists());
        assert!(logs_contain("Legacy main cannot be read"));
        assert!(logs_contain(&v1_uri(&files[2].hash(), "png")));
    }

    /// A takedown recorded under the `pubky.app` address of any file of a blob holds against
    /// the copy: the blob is not copied, a leftover copy is removed, and the blob's other file
    /// does not bring it back. The address list is the blob's key and each file's v0 address.
    #[tokio::test]
    async fn a_takedown_of_any_v0_file_of_a_blob_takes_the_blob_down() {
        let dir = TempDir::new().unwrap();
        let files = [
            LegacyFile::new("0032SSN7Q4EVG", b"copy:v0:shared", "image/png"),
            LegacyFile::new("0032SSN7Q4EVH", b"copy:v0:shared", "image/png"),
            LegacyFile::new("0032SSN7Q4EVJ", b"copy:v0:up", "image/png"),
        ];
        let map = seed(dir.path(), &files).await;
        let shared = files[0].hash();
        let leftover = v1_folder(dir.path(), &shared, "png");
        fs::create_dir_all(&leftover).await.unwrap();
        fs::write(main_path(&leftover), b"copy:v0:shared")
            .await
            .unwrap();
        // The moderator took down the second upload of the shared blob, at its v0 address.
        let down = v0_uri(files[1].tsid);
        let asked = Mutex::new(Vec::new());

        copy_with(&owner(), &map, dir.path(), |uris| {
            let hit = uris.contains(&down);
            asked.lock().unwrap().push(uris);
            async move { Ok(hit) }
        })
        .await
        .unwrap();

        assert!(!leftover.exists(), "the leftover copy must be removed");
        assert_eq!(
            v1_main(dir.path(), &files[2].hash(), "png")
                .await
                .as_deref(),
            Some(b"copy:v0:up".as_slice())
        );
        let asked = asked.into_inner().unwrap();
        assert!(
            asked.contains(&vec![
                blob_key(&shared),
                v0_uri(files[0].tsid),
                v0_uri(files[1].tsid),
            ]),
            "the shared blob is looked up under its key and both v0 addresses: {asked:?}"
        );
        assert_legacy_intact(dir.path(), &files).await;
    }
}
