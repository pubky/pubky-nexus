//! A `social/v1` file is raw bytes at `/pub/social/v1/files/{hash}.{ext}`, with no metadata
//! object and no node. A user at `v1` has it stored at `v1/<owner>/{hash}.{ext}/main` once its
//! bytes are checked against the hash in its address; a user at `v0` has it ignored. Prior art:
//! `create.rs`, `oversized.rs`, `source.rs` and the epoch gate tests in `users/epoch_gate.rs`.

use crate::event_processor::posts::utils::test_user;
use crate::event_processor::users::utils::set_user_epoch;
use crate::event_processor::utils::default_moderation_tests;
use crate::event_processor::utils::watcher::{retrieve_and_handle_event_line, WatcherTest};
use crate::service::utils::{new_in_memory_store, MockHomeserverClient};
use anyhow::Result;
use nexus_common::media::file_folder;
use nexus_common::models::event::EventLine;
use nexus_common::models::file::FileDetails;
use nexus_common::models::traits::Collection;
use nexus_common::models::user::UserIngestor;
use nexus_common::utils::test_utils::default_ingestor_tests;
use nexus_watcher::events::handlers::file::sync_put_v1;
use nexus_watcher::events::inputs::ResourceKey;
use nexus_watcher::events::retry::{IndexKey, RetryEvent, RetryScheduler};
use nexus_watcher::events::{DefaultEventHandler, EventHandler};
use nexus_watcher::EventProcessorError;
use pubky::{Keypair, ResourcePath};
use pubky_social_specs::traits::HashId;
use pubky_social_specs::{file_uri_builder, PubkyId, PubkySocialFile};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;

/// A cap small enough to reject a non-trivial file, as in `oversized.rs`.
const TINY_MAX_FILE_SIZE: u64 = 64;

/// A v1 file as a client writes it: its bytes, and the `{hash}.{ext}` leaf its address carries.
pub(super) struct V1File {
    bytes: Vec<u8>,
    filename: String,
}

impl V1File {
    /// The file of `bytes`, named by their hash and `ext`.
    pub(super) fn new(bytes: Vec<u8>, ext: &str) -> Self {
        let hash = PubkySocialFile(bytes.clone()).create_id();
        Self {
            bytes,
            filename: format!("{hash}.{ext}"),
        }
    }

    /// The same leaf, with another file's bytes behind it: what a homeserver, which checks no
    /// hash, can hold.
    fn with_bytes(&self, bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            filename: self.filename.clone(),
        }
    }

    pub(super) fn hash(&self) -> &str {
        self.filename
            .split_once('.')
            .expect("a {hash}.{ext} leaf")
            .0
    }

    /// The path of the file in its owner's storage.
    fn hs_path(&self) -> String {
        PubkySocialFile::create_path(&self.filename)
    }

    fn resource_path(&self) -> Result<ResourcePath> {
        Ok(self.hs_path().parse()?)
    }

    /// The file's address in `owner_id`'s storage.
    pub(super) fn uri(&self, owner_id: &str) -> String {
        file_uri_builder(owner_id.to_string(), self.filename.clone())
    }

    /// Where the watcher keeps the file's `main` under `files_path`.
    pub(super) fn main_path(&self, files_path: &Path, owner_id: &str) -> PathBuf {
        files_path
            .join(file_folder(owner_id, &self.filename))
            .join("main")
    }

    /// Writes the bytes to the owner's homeserver, without running the processor.
    async fn upload(&self, test: &mut WatcherTest, owner_kp: &Keypair) -> Result<()> {
        test.create_file_from_body(owner_kp, &self.hs_path(), self.bytes.clone())
            .await
    }

    /// Writes the bytes to the owner's homeserver and runs the processor over the PUT.
    pub(super) async fn put(&self, test: &mut WatcherTest, owner_kp: &Keypair) -> Result<()> {
        self.upload(test, owner_kp).await?;
        test.ensure_event_processing_complete().await
    }

    /// Deletes the file on the owner's homeserver and runs the processor over the DEL.
    pub(super) async fn del(&self, test: &mut WatcherTest, owner_kp: &Keypair) -> Result<()> {
        test.del(owner_kp, &self.resource_path()?).await
    }
}

/// A user at `v1` on the test homeserver, with their `pubky.app` profile indexed first so the
/// epoch is the only thing that differs from a v0 user. Standing in for the flip, as the epoch
/// gate tests do.
pub(super) async fn v1_user(test: &mut WatcherTest, name: &str) -> Result<(Keypair, String)> {
    let user_kp = Keypair::random();
    let user_id = test
        .create_user(&user_kp, &test_user(name, "v1_files"))
        .await?;
    set_user_epoch(&user_id, "v1").await?;
    Ok((user_kp, user_id))
}

/// Back to `v0`, so the gate lets the `pubky.app` cleanup through.
pub(super) async fn cleanup_v1_user(
    test: &mut WatcherTest,
    user_kp: &Keypair,
    user_id: &str,
) -> Result<()> {
    set_user_epoch(user_id, "v0").await?;
    test.cleanup_user(user_kp).await
}

/// Whether the shared event log holds `line`.
async fn event_line_stored(line: &str) -> Result<bool> {
    // Uncapped: the shared event log is append-only and unbounded.
    let (lines, _) = EventLine::get_from_index(None, usize::MAX).await?;
    Ok(lines.iter().any(|stored| stored == line))
}

/// Whether the retry queue holds an entry for `uri`.
async fn queued_for_retry(uri: &str) -> Result<bool> {
    Ok(RetryEvent::check_index_key(&IndexKey::for_uri(uri)).await?)
}

/// The names left in the watcher's v1 temp folder; a finished write leaves none.
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

/// The file is on disk as its `main`, with exactly its bytes, with no temp file left behind,
/// and no `File` node: a v1 file has no metadata.
async fn assert_stored(test: &WatcherTest, user_id: &str, file: &V1File) -> Result<()> {
    let main = file.main_path(test.temp_dir.path(), user_id);
    assert!(
        main.exists(),
        "{} must be on disk at {main:?}",
        file.filename
    );
    assert_eq!(
        fs::read(&main).await?,
        file.bytes,
        "{} must hold the uploaded bytes",
        file.filename
    );
    assert!(
        temp_files(test.temp_dir.path()).await.is_empty(),
        "a finished write leaves no temp file"
    );
    let nodes = FileDetails::get_by_ids(&[&[user_id, &file.filename]]).await?;
    assert!(
        nodes.is_empty() || nodes[0].is_none(),
        "a v1 file has no File node"
    );
    Ok(())
}

/// Nothing of the file is on disk: neither its folder nor a temp file.
async fn assert_not_stored(test: &WatcherTest, user_id: &str, file: &V1File) {
    let folder = test
        .temp_dir
        .path()
        .join(file_folder(user_id, &file.filename));
    assert!(
        !folder.exists(),
        "{} must not be on disk at {folder:?}",
        file.filename
    );
    assert!(
        temp_files(test.temp_dir.path()).await.is_empty(),
        "a dropped file leaves no temp file"
    );
}

/// A handler over a homeserver that serves nothing: every blob fetch is a 404 and is recorded,
/// so a test can tell whether the handler asked for the bytes at all.
pub(super) fn handler_over_mock(
    files_path: &Path,
) -> (Arc<dyn EventHandler>, Arc<MockHomeserverClient>) {
    let client = Arc::new(MockHomeserverClient::default());
    let handler: Arc<dyn EventHandler> = Arc::new(DefaultEventHandler::new(
        default_moderation_tests(),
        default_ingestor_tests(),
        client.clone(),
        files_path.to_path_buf(),
        new_in_memory_store(),
    ));
    (handler, client)
}

/// A v1 file PUT of a user at `v0` is ignored: nothing on disk, no event line, no retry entry.
/// The same PUT once the user is at `v1` stores the file at its folder.
#[tokio_shared_rt::test(shared)]
async fn test_v1_file_is_ignored_at_v0_and_stored_at_v1() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let user_kp = Keypair::random();
    let user_id = test
        .create_user(&user_kp, &test_user("Watcher:V1File:Gate", "v1_files"))
        .await?;
    let file = V1File::new(b"Watcher:V1File:Gate:Bytes".to_vec(), "bin");
    let uri = file.uri(&user_id);

    file.put(&mut test, &user_kp).await?;

    assert_not_stored(&test, &user_id, &file).await;
    assert!(
        !event_line_stored(&format!("PUT {uri}")).await?,
        "an ignored event must store no event line"
    );
    assert!(!queued_for_retry(&uri).await?);

    set_user_epoch(&user_id, "v1").await?;
    file.put(&mut test, &user_kp).await?;

    assert_stored(&test, &user_id, &file).await?;
    assert!(event_line_stored(&format!("PUT {uri}")).await?);
    assert!(
        !test.temp_dir.path().join(&user_id).exists(),
        "a v1 file must not create a legacy owner folder"
    );

    file.del(&mut test, &user_kp).await?;
    cleanup_v1_user(&mut test, &user_kp, &user_id).await
}

/// A v1 file larger than the 2 MB cap every JSON resource is fetched under is stored: the
/// handler downloads it under the file cap instead.
#[tokio_shared_rt::test(shared)]
async fn test_v1_file_over_the_resource_cap_is_stored() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (user_kp, user_id) = v1_user(&mut test, "Watcher:V1File:Large").await?;
    // Over the resource cap, with content that is not one repeated byte.
    let bytes: Vec<u8> = (0..(2 * 1024 * 1024 + 4096))
        .map(|i| (i % 251) as u8)
        .collect();
    let file = V1File::new(bytes, "bin");

    file.put(&mut test, &user_kp).await?;

    assert_stored(&test, &user_id, &file).await?;

    file.del(&mut test, &user_kp).await?;
    cleanup_v1_user(&mut test, &user_kp, &user_id).await
}

/// A v1 file over the file cap is dropped as `FetchSizeExceeded`: nothing on disk, and no retry
/// entry, through the handler and through the processor alike.
#[tokio_shared_rt::test(shared)]
async fn test_v1_file_over_the_file_cap_is_dropped_and_not_retried() -> Result<()> {
    let mut test = WatcherTest::setup(Some(TINY_MAX_FILE_SIZE)).await?;
    let (user_kp, user_id) = v1_user(&mut test, "Watcher:V1File:OverCap").await?;
    let file = V1File::new(vec![0xAB; TINY_MAX_FILE_SIZE as usize + 1], "bin");
    let uri = file.uri(&user_id);

    // Handle the PUT directly, so the error class is observable.
    test = test.remove_event_processing().await;
    file.upload(&mut test, &user_kp).await?;
    let err = retrieve_and_handle_event_line(
        &format!("PUT {uri}"),
        test.event_processor_runner.event_handler.clone(),
    )
    .await
    .expect_err("a file over the cap must be rejected");

    assert!(
        matches!(
            err,
            EventProcessorError::FetchSizeExceeded(_, TINY_MAX_FILE_SIZE)
        ),
        "expected FetchSizeExceeded, got {err:?}"
    );
    assert!(
        !RetryScheduler::should_enqueue_related_event(&err),
        "a file over the cap must not be retried"
    );
    assert_not_stored(&test, &user_id, &file).await;

    // The processor drops it the same way.
    test.ensure_event_processing = true;
    test.ensure_event_processing_complete().await?;
    assert_not_stored(&test, &user_id, &file).await;
    assert!(!queued_for_retry(&uri).await?);

    file.del(&mut test, &user_kp).await?;
    cleanup_v1_user(&mut test, &user_kp, &user_id).await
}

/// Bytes that don't hash to the address are dropped as spec validation: nothing is written,
/// and the event is not retried, since the same bytes fail the same way.
#[tokio_shared_rt::test(shared)]
async fn test_v1_file_whose_bytes_do_not_match_its_hash_is_dropped() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (user_kp, user_id) = v1_user(&mut test, "Watcher:V1File:Mismatch").await?;
    // The homeserver checks no hash: the leaf of one file holds the bytes of another.
    let file = V1File::new(b"Watcher:V1File:Mismatch:Named".to_vec(), "png")
        .with_bytes(b"Watcher:V1File:Mismatch:Stored".to_vec());
    let uri = file.uri(&user_id);

    test = test.remove_event_processing().await;
    file.upload(&mut test, &user_kp).await?;
    let err = retrieve_and_handle_event_line(
        &format!("PUT {uri}"),
        test.event_processor_runner.event_handler.clone(),
    )
    .await
    .expect_err("bytes that don't match the hash must be rejected");

    assert!(
        matches!(err, EventProcessorError::SpecValidation(_)),
        "expected SpecValidation, got {err:?}"
    );
    assert!(
        !RetryScheduler::should_enqueue_related_event(&err),
        "a hash mismatch must not be retried"
    );
    assert_not_stored(&test, &user_id, &file).await;

    test.ensure_event_processing = true;
    test.ensure_event_processing_complete().await?;
    assert_not_stored(&test, &user_id, &file).await;
    assert!(!queued_for_retry(&uri).await?);

    file.del(&mut test, &user_kp).await?;
    cleanup_v1_user(&mut test, &user_kp, &user_id).await
}

/// `H.png` and `H.bin` are two files on the homeserver, so a DEL of one removes its folder and
/// leaves the other, and the deleted one can be uploaded again.
#[tokio_shared_rt::test(shared)]
async fn test_v1_file_del_removes_only_its_folder_and_a_reupload_works() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (user_kp, user_id) = v1_user(&mut test, "Watcher:V1File:Del").await?;
    let bytes = b"Watcher:V1File:Del:Bytes".to_vec();
    let png = V1File::new(bytes.clone(), "png");
    let bin = V1File::new(bytes, "bin");
    png.put(&mut test, &user_kp).await?;
    bin.put(&mut test, &user_kp).await?;
    assert_stored(&test, &user_id, &png).await?;
    assert_stored(&test, &user_id, &bin).await?;

    png.del(&mut test, &user_kp).await?;

    assert_not_stored(&test, &user_id, &png).await;
    assert_stored(&test, &user_id, &bin).await?;
    assert!(
        event_line_stored(&format!("DEL {}", png.uri(&user_id))).await?,
        "a handled DEL stores its event line"
    );

    png.put(&mut test, &user_kp).await?;
    assert_stored(&test, &user_id, &png).await?;

    png.del(&mut test, &user_kp).await?;
    bin.del(&mut test, &user_kp).await?;
    cleanup_v1_user(&mut test, &user_kp, &user_id).await
}

/// A v1 file whose owner is hosted on a blacklisted homeserver is refused before any download:
/// `HsBlacklisted`, nothing on disk.
#[tokio_shared_rt::test(shared)]
async fn test_v1_file_of_a_blacklisted_homeserver_is_refused() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (user_kp, user_id) = v1_user(&mut test, "Watcher:V1File:Blacklist").await?;
    let file = V1File::new(b"Watcher:V1File:Blacklist:Bytes".to_vec(), "bin");
    file.upload(&mut test, &user_kp).await?;

    // An ingestor blacklisting the homeserver that hosts the owner: the test homeserver.
    let ingestor = UserIngestor::new(
        [test.homeserver_id.clone()],
        test.event_processor_runner.client.clone(),
    );
    let key = ResourceKey {
        owner_id: PubkyId::from(user_kp.public_key()),
        resource_id: file.filename.clone(),
    };
    let err = sync_put_v1(
        key,
        file.hash(),
        test.temp_dir.path(),
        &ingestor,
        test.event_processor_runner.client.as_ref(),
    )
    .await
    .expect_err("a v1 file of a blacklisted homeserver must be refused");

    assert!(
        matches!(&err, EventProcessorError::HsBlacklisted { hs_id } if *hs_id == *test.homeserver_id),
        "expected HsBlacklisted for {}, got {err:?}",
        test.homeserver_id
    );
    assert!(
        !RetryScheduler::should_enqueue_related_event(&err),
        "a blacklisted homeserver must not be retried"
    );
    assert_not_stored(&test, &user_id, &file).await;

    file.del(&mut test, &user_kp).await?;
    cleanup_v1_user(&mut test, &user_kp, &user_id).await
}

/// A replayed PUT of a file already on disk fetches nothing: an address names exact bytes and
/// every write checked them. A replayed PUT of a file no longer on disk fills it in.
#[tokio_shared_rt::test(shared)]
async fn test_v1_file_replay_fetches_nothing_when_main_exists_and_fills_a_missing_one() -> Result<()>
{
    let mut test = WatcherTest::setup(None).await?;
    let (user_kp, user_id) = v1_user(&mut test, "Watcher:V1File:Replay").await?;
    let file = V1File::new(b"Watcher:V1File:Replay:Bytes".to_vec(), "bin");
    let line = format!("PUT {}", file.uri(&user_id));
    file.put(&mut test, &user_kp).await?;
    assert_stored(&test, &user_id, &file).await?;

    // Over a homeserver that serves nothing: a replay that fetched would fail.
    let (handler, client) = handler_over_mock(test.temp_dir.path());
    retrieve_and_handle_event_line(&line, handler.clone())
        .await
        .expect("a replay of a file on disk succeeds without a fetch");
    assert!(
        client.blob_fetches().await.is_empty(),
        "a replay of a file on disk must fetch nothing, got {:?}",
        client.blob_fetches().await
    );
    assert_stored(&test, &user_id, &file).await?;

    // Gone from disk: the same replay over the same homeserver now asks for the bytes.
    let folder = test
        .temp_dir
        .path()
        .join(file_folder(&user_id, &file.filename));
    fs::remove_dir_all(&folder).await?;
    retrieve_and_handle_event_line(&line, handler)
        .await
        .expect_err("a replay of a missing file fetches, and this homeserver has nothing");
    assert_eq!(client.blob_fetches().await, vec![file.uri(&user_id)]);

    // Over the real homeserver, the replay fills the file in.
    retrieve_and_handle_event_line(&line, test.event_processor_runner.event_handler.clone())
        .await
        .expect("a replay of a missing file fills it in");
    assert_stored(&test, &user_id, &file).await?;

    file.del(&mut test, &user_kp).await?;
    cleanup_v1_user(&mut test, &user_kp, &user_id).await
}
