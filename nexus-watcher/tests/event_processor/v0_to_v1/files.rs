//! The copy step: a migrated user's files are at their `social/v1` addresses after the flip,
//! copied from the legacy folders, which stay in place. Driven through the event-processing
//! harness, checked on disk and in the graph. Prior art: `attachments.rs` and the file
//! takedown tests in `files/takedown.rs`.

use super::attachments::{create_file, upload_blob, v1_file_uri};
use crate::event_processor::posts::utils::{short_post, test_user, v1_test_user};
use crate::event_processor::users::utils::{find_user_epoch, set_user_epoch};
use crate::event_processor::utils::watcher::{
    retrieve_and_handle_event_line, HomeserverHashIdPath, WatcherTest,
};
use anyhow::Result;
use chrono::Utc;
use nexus_common::media::file_folder;
use nexus_common::models::post::PostView;
use nexus_common::models::takedown::is_taken_down;
use nexus_watcher::events::handlers::file::blob_takedown_key;
use nexus_watcher::EventProcessorError;
use pubky::{recovery_file, Keypair, ResourcePath};
use pubky_social_specs::legacy_v0::{PubkyAppPost, PubkyAppTag};
use std::path::{Path, PathBuf};
use tokio::fs;

const MODERATION_LABEL: &str = "label_to_moderate";

/// A `pubky.app` file of the author, indexed: the blob it names, its id, its path on the
/// homeserver, its address and its legacy `main` on disk.
struct LegacyFile {
    hash: String,
    src: String,
    bytes: Vec<u8>,
    file_id: String,
    path: ResourcePath,
    uri: String,
    main: PathBuf,
}

/// Uploads `content` as the author's blob and writes a `File` object naming it.
async fn legacy_file(
    test: &mut WatcherTest,
    user_kp: &Keypair,
    user_id: &str,
    content: &str,
    name: &str,
    content_type: &str,
) -> Result<LegacyFile> {
    let (hash, src) = upload_blob(test, user_kp, user_id, content).await?;
    let mut file = legacy_file_naming(test, user_kp, user_id, &src, name, content_type).await?;
    file.hash = hash;
    file.bytes = content.as_bytes().to_vec();
    Ok(file)
}

/// Writes a `File` object naming the blob at `src`, a second upload of one the author already
/// has. Its `hash` and `bytes` are left empty: a test reads them from the first upload.
async fn legacy_file_naming(
    test: &mut WatcherTest,
    user_kp: &Keypair,
    user_id: &str,
    src: &str,
    name: &str,
    content_type: &str,
) -> Result<LegacyFile> {
    let (file_id, path, uri) = create_file(test, user_kp, user_id, name, src, content_type).await?;
    let main = test
        .temp_dir
        .path()
        .join(file_folder(user_id, &file_id))
        .join("main");
    assert!(
        main.exists(),
        "the legacy main of {file_id} must be on disk"
    );
    Ok(LegacyFile {
        hash: String::new(),
        src: src.to_string(),
        bytes: Vec::new(),
        file_id,
        path,
        uri,
        main,
    })
}

/// Where the flip copies the author's blob `hash` under `ext`.
fn v1_main(files_path: &Path, user_id: &str, hash: &str, ext: &str) -> PathBuf {
    files_path
        .join(file_folder(user_id, &format!("{hash}.{ext}")))
        .join("main")
}

/// The names left in the watcher's v1 temp folder; a finished copy leaves none.
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

/// The author's `pubky.app` profile, indexed.
async fn v0_author(test: &mut WatcherTest, name: &str) -> Result<(Keypair, String)> {
    let user_kp = Keypair::random();
    let user_id = test
        .create_user(&user_kp, &test_user(name, "migration"))
        .await?;
    Ok((user_kp, user_id))
}

/// The flip: the author's `social/v1` profile, read by the processor.
async fn flip(test: &mut WatcherTest, user_kp: &Keypair, name: &str) -> Result<ResourcePath> {
    test.put_v1_profile(user_kp, &v1_test_user(name, "migration"))
        .await
}

/// Back to `v0`, so the gate lets the `pubky.app` cleanup through; the v1 DEL is then ignored.
async fn cleanup(
    test: &mut WatcherTest,
    user_kp: &Keypair,
    user_id: &str,
    profile_path: &ResourcePath,
    files: &[&LegacyFile],
    posts: &[&ResourcePath],
) -> Result<()> {
    set_user_epoch(user_id, "v0").await?;
    test.del(user_kp, profile_path).await?;
    for path in posts {
        test.cleanup_post(user_kp, path).await?;
    }
    for file in files {
        test.cleanup_file(user_kp, &file.path).await?;
    }
    test.cleanup_user(user_kp).await
}

/// The moderator the test handler trusts, at `v0`, so their `pubky.app` tags are handled.
async fn create_moderator(test: &mut WatcherTest) -> Result<Keypair> {
    let recovery = fs::read("./tests/event_processor/utils/moderator_key.pkarr").await?;
    let moderator_kp = recovery_file::decrypt_recovery_file(&recovery, "password")?;
    test.create_user(&moderator_kp, &test_user("Mod:Moderator", "moderator"))
        .await?;
    Ok(moderator_kp)
}

/// The moderator tags `target_uri` with the moderated label, and the processor reads it.
async fn moderate(test: &mut WatcherTest, moderator_kp: &Keypair, target_uri: &str) -> Result<()> {
    let tag = PubkyAppTag {
        uri: target_uri.to_string(),
        label: MODERATION_LABEL.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    test.put(moderator_kp, &tag.hs_path(), tag).await
}

/// The flip copies each blob once, under its namer's extension, and leaves every legacy folder
/// in place: two uploads of one blob give one copy, and attachments naming either upload resolve
/// to it.
#[tokio_shared_rt::test(shared)]
async fn test_flip_copies_each_file_once_and_leaves_the_legacy_folders_in_place() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (author_kp, author_id) = v0_author(&mut test, "Watcher:Migration:Copy:Author").await?;
    let unique = format!("{author_id}:{}", Utc::now().timestamp_micros());
    let image = legacy_file(
        &mut test,
        &author_kp,
        &author_id,
        &format!("shared:{unique}"),
        "shared.png",
        "image/png",
    )
    .await?;
    let text = legacy_file_naming(
        &mut test,
        &author_kp,
        &author_id,
        &image.src,
        "shared.txt",
        "text/plain",
    )
    .await?;
    assert!(image.file_id < text.file_id, "the image is the lowest tsid");
    let alone = legacy_file(
        &mut test,
        &author_kp,
        &author_id,
        &format!("alone:{unique}"),
        "alone.bin",
        "application/octet-stream",
    )
    .await?;
    let (post_id, post_path) = test
        .create_post(
            &author_kp,
            &PubkyAppPost {
                attachments: Some(vec![image.uri.clone(), text.uri.clone()]),
                ..short_post("Watcher:Migration:Copy:Post")
            },
        )
        .await?;

    let profile_path = flip(&mut test, &author_kp, "Watcher:Migration:Copy:Author:V1").await?;

    assert_eq!(find_user_epoch(&author_id).await.as_deref(), Some("v1"));
    let files_path = test.temp_dir.path();
    assert_eq!(
        fs::read(v1_main(files_path, &author_id, &image.hash, "png")).await?,
        image.bytes
    );
    assert!(
        !files_path
            .join(file_folder(&author_id, &format!("{}.txt", image.hash)))
            .exists(),
        "one blob gives one copy, under the lowest tsid's extension"
    );
    assert_eq!(
        fs::read(v1_main(files_path, &author_id, &alone.hash, "bin")).await?,
        alone.bytes
    );
    for file in [&image, &text, &alone] {
        assert!(
            file.main.exists(),
            "the legacy folder of {} must be left in place",
            file.file_id
        );
    }
    assert_eq!(fs::read(&text.main).await?, image.bytes);
    assert!(temp_files(files_path).await.is_empty());
    let served = PostView::get_by_id(&author_id, &post_id, None, None, None)
        .await?
        .expect("the post is served");
    let copied = v1_file_uri(&author_id, &image.hash, "png");
    assert_eq!(
        served.details.attachments,
        Some(vec![copied.clone(), copied]),
        "attachments naming either upload resolve to the one copy"
    );

    cleanup(
        &mut test,
        &author_kp,
        &author_id,
        &profile_path,
        &[&image, &text, &alone],
        &[&post_path],
    )
    .await
}

/// A blob whose legacy `main` holds other bytes, and one whose `main` is gone, are skipped; the
/// rest are copied and the user reaches `v1`.
#[tokio_shared_rt::test(shared)]
async fn test_flip_skips_a_bad_and_a_missing_source_and_reaches_v1() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (author_kp, author_id) = v0_author(&mut test, "Watcher:Migration:Skip:Author").await?;
    let unique = format!("{author_id}:{}", Utc::now().timestamp_micros());
    let mut files = Vec::new();
    for name in ["good", "bad", "missing"] {
        files.push(
            legacy_file(
                &mut test,
                &author_kp,
                &author_id,
                &format!("{name}:{unique}"),
                &format!("{name}.png"),
                "image/png",
            )
            .await?,
        );
    }
    let [good, bad, missing] = files.as_slice() else {
        unreachable!()
    };
    fs::write(&bad.main, b"not the blob").await?;
    fs::remove_dir_all(missing.main.parent().unwrap()).await?;

    let profile_path = flip(&mut test, &author_kp, "Watcher:Migration:Skip:Author:V1").await?;

    assert_eq!(find_user_epoch(&author_id).await.as_deref(), Some("v1"));
    let files_path = test.temp_dir.path();
    assert_eq!(
        fs::read(v1_main(files_path, &author_id, &good.hash, "png")).await?,
        good.bytes
    );
    for file in [bad, missing] {
        assert!(
            !v1_main(files_path, &author_id, &file.hash, "png").exists(),
            "{} must be skipped",
            file.file_id
        );
    }
    assert_eq!(fs::read(&bad.main).await?, b"not the blob");
    assert!(temp_files(files_path).await.is_empty());

    cleanup(
        &mut test,
        &author_kp,
        &author_id,
        &profile_path,
        &[good, bad, missing],
        &[],
    )
    .await
}

/// A group whose first `tsid` holds other bytes is copied from a later one that passes, still
/// under the first one's extension.
#[tokio_shared_rt::test(shared)]
async fn test_flip_copies_a_group_from_a_later_tsid_when_the_first_is_bad() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (author_kp, author_id) = v0_author(&mut test, "Watcher:Migration:Group:Author").await?;
    let unique = format!("{author_id}:{}", Utc::now().timestamp_micros());
    let first = legacy_file(
        &mut test,
        &author_kp,
        &author_id,
        &format!("group:{unique}"),
        "group.png",
        "image/png",
    )
    .await?;
    let second = legacy_file_naming(
        &mut test,
        &author_kp,
        &author_id,
        &first.src,
        "group.txt",
        "text/plain",
    )
    .await?;
    assert!(first.file_id < second.file_id);
    fs::write(&first.main, b"not the blob").await?;

    let profile_path = flip(&mut test, &author_kp, "Watcher:Migration:Group:Author:V1").await?;

    assert_eq!(find_user_epoch(&author_id).await.as_deref(), Some("v1"));
    let files_path = test.temp_dir.path();
    assert_eq!(
        fs::read(v1_main(files_path, &author_id, &first.hash, "png")).await?,
        first.bytes,
        "copied from the second upload, under the first one's extension"
    );
    assert!(!v1_main(files_path, &author_id, &first.hash, "txt").exists());

    cleanup(
        &mut test,
        &author_kp,
        &author_id,
        &profile_path,
        &[&first, &second],
        &[],
    )
    .await
}

/// A write that fails is the migration's disk error: the flip fails and the user stays at `v0`,
/// with nothing under the final name. The retry, once the disk is repaired, copies the rest and
/// rewrites nothing that exists.
#[tokio_shared_rt::test(shared)]
async fn test_a_write_failure_leaves_the_user_at_v0_and_the_retry_rewrites_nothing() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (author_kp, author_id) = v0_author(&mut test, "Watcher:Migration:Disk:Author").await?;
    let unique = format!("{author_id}:{}", Utc::now().timestamp_micros());
    let mut files = Vec::new();
    for name in ["a", "b"] {
        files.push(
            legacy_file(
                &mut test,
                &author_kp,
                &author_id,
                &format!("{name}:{unique}"),
                &format!("{name}.png"),
                "image/png",
            )
            .await?,
        );
    }
    let [a, b] = files.as_slice() else {
        unreachable!()
    };
    let files_path = test.temp_dir.path().to_path_buf();
    let a_main = v1_main(&files_path, &author_id, &a.hash, "png");
    let b_main = v1_main(&files_path, &author_id, &b.hash, "png");
    // A file where the writer's temp folder goes: no write can start, as on a full disk.
    let blocked = files_path.join("v1").join(".tmp");
    fs::create_dir_all(files_path.join("v1")).await?;
    fs::write(&blocked, b"").await?;

    // The flip, handled directly, so the error class is observable.
    test = test.remove_event_processing().await;
    let profile_path = flip(&mut test, &author_kp, "Watcher:Migration:Disk:Author:V1").await?;
    let line = format!("PUT pubky://{author_id}{profile_path}");
    let handler = test.event_processor_runner.event_handler.clone();
    let err = retrieve_and_handle_event_line(&line, handler.clone())
        .await
        .expect_err("a failed write must fail the flip");

    assert!(
        matches!(
            &err,
            EventProcessorError::FlipFailed { step: "copy", source }
                if matches!(**source, EventProcessorError::MigrationDiskFailed(_))
        ),
        "expected the copy step's MigrationDiskFailed, got {err:?}"
    );
    assert!(
        !err.should_not_retry_now(),
        "a failed copy is the flip's own retry, never a batch stop"
    );
    // A `pubky.app` user's node carries no epoch until the flip writes `v1`.
    assert_ne!(
        find_user_epoch(&author_id).await.as_deref(),
        Some("v1"),
        "the user stays at v0"
    );
    for main in [&a_main, &b_main] {
        assert!(
            !main.parent().unwrap().exists(),
            "nothing was written under the final name"
        );
    }

    // The disk repaired, with one copy already there, the retry finishes the flip and rewrites
    // nothing that exists.
    fs::remove_file(&blocked).await?;
    fs::create_dir_all(a_main.parent().unwrap()).await?;
    fs::write(&a_main, b"sentinel").await?;
    retrieve_and_handle_event_line(&line, handler)
        .await
        .expect("the retry finishes the flip");

    assert_eq!(find_user_epoch(&author_id).await.as_deref(), Some("v1"));
    assert_eq!(fs::read(&a_main).await?, b"sentinel");
    assert_eq!(fs::read(&b_main).await?, b.bytes);
    assert!(temp_files(&files_path).await.is_empty());
    for file in [a, b] {
        assert!(file.main.exists(), "the legacy folders are left in place");
    }

    // The processor reads the backlog, the profile PUT of a user now at `v1` among it.
    test.ensure_event_processing = true;
    test.ensure_event_processing_complete().await?;
    cleanup(
        &mut test,
        &author_kp,
        &author_id,
        &profile_path,
        &[a, b],
        &[],
    )
    .await
}

/// A v1 address taken down before the flip is not copied; the flip goes on with the rest, and
/// the legacy folder stays.
#[tokio_shared_rt::test(shared)]
async fn test_a_v1_address_taken_down_before_the_flip_is_not_copied() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let moderator_kp = create_moderator(&mut test).await?;
    let (author_kp, author_id) = v0_author(&mut test, "Watcher:Migration:Down:Author").await?;
    let unique = format!("{author_id}:{}", Utc::now().timestamp_micros());
    let down = legacy_file(
        &mut test,
        &author_kp,
        &author_id,
        &format!("down:{unique}"),
        "down.png",
        "image/png",
    )
    .await?;
    let up = legacy_file(
        &mut test,
        &author_kp,
        &author_id,
        &format!("up:{unique}"),
        "up.png",
        "image/png",
    )
    .await?;

    moderate(
        &mut test,
        &moderator_kp,
        &v1_file_uri(&author_id, &down.hash, "png"),
    )
    .await?;
    let profile_path = flip(&mut test, &author_kp, "Watcher:Migration:Down:Author:V1").await?;

    assert_eq!(find_user_epoch(&author_id).await.as_deref(), Some("v1"));
    let files_path = test.temp_dir.path();
    assert!(
        !v1_main(files_path, &author_id, &down.hash, "png").exists(),
        "a taken-down address must not be copied"
    );
    assert_eq!(
        fs::read(v1_main(files_path, &author_id, &up.hash, "png")).await?,
        up.bytes
    );
    assert!(down.main.exists(), "the legacy folder is left alone");

    cleanup(
        &mut test,
        &author_kp,
        &author_id,
        &profile_path,
        &[&down, &up],
        &[],
    )
    .await
}

/// A `{tsid}` takedown before the flip takes its blob down under its key too, so the blob is not
/// copied, even when another upload the moderator did not tag shares it: the bytes are the
/// content taken down. The untagged upload declares another type, so once the tagged upload's
/// node is gone it names the blob and the flip would copy it as `H.txt` where the takedown saw
/// `H.png`: the key carries no extension, so neither is copied. That upload's legacy folder is
/// left alone.
#[tokio_shared_rt::test(shared)]
async fn test_a_tsid_takedown_before_the_flip_takes_its_blob_down_shared_or_not() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let moderator_kp = create_moderator(&mut test).await?;
    let (author_kp, author_id) = v0_author(&mut test, "Watcher:Migration:Tsid:Author").await?;
    let unique = format!("{author_id}:{}", Utc::now().timestamp_micros());
    let lone = legacy_file(
        &mut test,
        &author_kp,
        &author_id,
        &format!("lone:{unique}"),
        "lone.png",
        "image/png",
    )
    .await?;
    let shared = legacy_file(
        &mut test,
        &author_kp,
        &author_id,
        &format!("shared:{unique}"),
        "shared.png",
        "image/png",
    )
    .await?;
    let other = legacy_file_naming(
        &mut test,
        &author_kp,
        &author_id,
        &shared.src,
        "shared-again.txt",
        "text/plain",
    )
    .await?;

    moderate(&mut test, &moderator_kp, &lone.uri).await?;
    moderate(&mut test, &moderator_kp, &shared.uri).await?;
    assert!(!lone.main.exists() && !shared.main.exists());
    assert!(other.main.exists());
    let profile_path = flip(&mut test, &author_kp, "Watcher:Migration:Tsid:Author:V1").await?;

    assert_eq!(find_user_epoch(&author_id).await.as_deref(), Some("v1"));
    let files_path = test.temp_dir.path();
    for (file, what) in [(&lone, "a lone blob"), (&shared, "a shared blob")] {
        for ext in ["png", "txt"] {
            assert!(
                !v1_main(files_path, &author_id, &file.hash, ext).exists(),
                "{what} of a taken-down tsid must not be copied, as {ext} or anything else"
            );
        }
        assert!(
            is_taken_down(&blob_takedown_key(&author_id, &file.hash)).await?,
            "{what}: the takedown must be recorded under the blob's key"
        );
    }
    assert!(
        other.main.exists(),
        "the untagged upload's legacy folder stays"
    );

    cleanup(
        &mut test,
        &author_kp,
        &author_id,
        &profile_path,
        &[&lone, &shared, &other],
        &[],
    )
    .await
}

/// A `{tsid}` takedown read after the flip removes the copy as well as the legacy folder, and
/// records the copy's address, so the copy does not outlive the takedown.
#[tokio_shared_rt::test(shared)]
async fn test_a_tsid_takedown_after_the_flip_removes_the_copy() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let moderator_kp = create_moderator(&mut test).await?;
    let (author_kp, author_id) = v0_author(&mut test, "Watcher:Migration:After:Author").await?;
    let unique = format!("{author_id}:{}", Utc::now().timestamp_micros());
    let down = legacy_file(
        &mut test,
        &author_kp,
        &author_id,
        &format!("after:down:{unique}"),
        "down.png",
        "image/png",
    )
    .await?;
    let up = legacy_file(
        &mut test,
        &author_kp,
        &author_id,
        &format!("after:up:{unique}"),
        "up.png",
        "image/png",
    )
    .await?;
    let profile_path = flip(&mut test, &author_kp, "Watcher:Migration:After:Author:V1").await?;
    let files_path = test.temp_dir.path().to_path_buf();
    let copy = v1_main(&files_path, &author_id, &down.hash, "png");
    assert!(copy.exists(), "the flip copies the file");

    moderate(&mut test, &moderator_kp, &down.uri).await?;

    assert!(!down.main.exists(), "the legacy folder is removed");
    assert!(!copy.exists(), "the copy is removed with it");
    assert!(
        is_taken_down(&blob_takedown_key(&author_id, &down.hash)).await?,
        "the blob's key is recorded, so a later PUT of its bytes stays down"
    );
    assert_eq!(
        fs::read(v1_main(&files_path, &author_id, &up.hash, "png")).await?,
        up.bytes,
        "the other file's copy stays"
    );

    cleanup(
        &mut test,
        &author_kp,
        &author_id,
        &profile_path,
        &[&down, &up],
        &[],
    )
    .await
}

/// A user whose first event is a `social/v1` profile PUT flips with nothing to copy: no folder
/// of theirs appears on disk.
#[tokio_shared_rt::test(shared)]
async fn test_a_first_time_social_user_flips_with_nothing_to_copy() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let user_kp = Keypair::random();
    let user_id = user_kp.public_key().to_z32();
    test.register_user(&user_kp).await?;

    let profile_path = flip(&mut test, &user_kp, "Watcher:Migration:Fresh:V1").await?;

    assert_eq!(find_user_epoch(&user_id).await.as_deref(), Some("v1"));
    let files_path = test.temp_dir.path();
    assert!(!files_path.join("v1").join(&user_id).exists());
    assert!(!files_path.join(&user_id).exists());

    // Handled at `v1`
    test.del(&user_kp, &profile_path).await
}
