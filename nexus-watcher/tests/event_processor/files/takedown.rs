//! One takedown covers one address. A moderator tag on a `{hash}.{ext}` address removes that
//! v1 folder and nothing else, and a later PUT at the address stays down; a tag on a `{tsid}`
//! address removes that legacy folder and its `File` node and nothing else. Prior art:
//! `tags/moderated.rs` and `delete.rs`.

use super::social_v1::{cleanup_v1_user, handler_over_mock, v1_user, V1File};
use crate::event_processor::posts::utils::test_user;
use crate::event_processor::users::utils::set_user_epoch;
use crate::event_processor::utils::watcher::{
    assert_file_details, retrieve_and_handle_event_line, HomeserverHashIdPath, WatcherTest,
};
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::file::FileDetails;
use nexus_common::models::takedown::is_taken_down;
use nexus_common::models::traits::Collection;
use pubky::{recovery_file, Keypair, ResourcePath};
use pubky_social_specs::legacy_v0::traits::{HasIdPath, HashId as _};
use pubky_social_specs::legacy_v0::{
    blob_uri_builder, file_uri_builder as v0_file_uri_builder, PubkyAppBlob, PubkyAppFile,
    PubkyAppTag,
};
use std::path::PathBuf;
use tokio::fs;

const MODERATION_LABEL: &str = "label_to_moderate";

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

/// A `pubky.app` file of `user_kp`, indexed: its id, its address and its `main` on disk.
async fn create_v0_file(
    test: &mut WatcherTest,
    user_kp: &Keypair,
    user_id: &str,
    content: &str,
) -> Result<(String, String, PathBuf)> {
    let blob = PubkyAppBlob::new(content.as_bytes().to_vec());
    let blob_id = blob.create_id();
    let blob_uri = blob_uri_builder(user_id.to_string(), blob_id.clone());
    test.create_file_from_body(
        user_kp,
        &PubkyAppBlob::create_path(&blob_id),
        blob.0.clone(),
    )
    .await?;
    let file = PubkyAppFile {
        name: "takedown".to_string(),
        content_type: "text/plain".to_string(),
        src: blob_uri.clone(),
        size: blob.0.len(),
        created_at: Utc::now().timestamp_millis(),
    };
    let (file_id, _) = test.create_file(user_kp, &file).await?;
    let details = assert_file_details(user_id, &file_id, &blob_uri, &file).await;
    let main = test.temp_dir.path().join(&details.urls.main);
    assert!(main.exists(), "the v0 file's main must be on disk");
    Ok((
        file_id.clone(),
        v0_file_uri_builder(user_id.to_string(), file_id),
        main,
    ))
}

/// A `social/v1` file of `user_kp`, who must be at `v1`, indexed: the file, its address and
/// its `main` on disk.
async fn create_v1_file(
    test: &mut WatcherTest,
    user_kp: &Keypair,
    user_id: &str,
    content: &str,
) -> Result<(V1File, String, PathBuf)> {
    let file = V1File::new(content.as_bytes().to_vec(), "png");
    file.put(test, user_kp).await?;
    let main = file.main_path(test.temp_dir.path(), user_id);
    assert!(main.exists(), "the v1 file's main must be on disk");
    let uri = file.uri(user_id);
    Ok((file, uri, main))
}

/// Whether the owner's `File` node for `file_id` exists.
async fn v0_file_node_exists(user_id: &str, file_id: &str) -> Result<bool> {
    let nodes = FileDetails::get_by_ids(&[&[user_id, file_id]]).await?;
    Ok(!nodes.is_empty() && nodes[0].is_some())
}

/// A tag on a v1 file's address removes its folder and records the takedown, so a later PUT at
/// the address is skipped before any fetch and the file stays down.
#[tokio_shared_rt::test(shared)]
async fn test_takedown_of_a_v1_file_removes_its_folder_and_holds_against_a_later_put() -> Result<()>
{
    let mut test = WatcherTest::setup(None).await?;
    let moderator_kp = create_moderator(&mut test).await?;
    let (author_kp, author_id) = v1_user(&mut test, "Watcher:V1File:Takedown").await?;
    let (file, uri, main) =
        create_v1_file(&mut test, &author_kp, &author_id, "Watcher:V1File:Takedown").await?;

    moderate(&mut test, &moderator_kp, &uri).await?;

    assert!(!main.exists(), "the takedown must remove the v1 folder");
    assert!(is_taken_down(&uri).await?, "the takedown must be recorded");

    // A later PUT at the address, over a homeserver that serves nothing: skipped before any
    // fetch, so it succeeds and leaves the file down.
    let (handler, client) = handler_over_mock(test.temp_dir.path());
    retrieve_and_handle_event_line(&format!("PUT {uri}"), handler)
        .await
        .expect("a PUT of a taken-down file is skipped, not failed");
    assert!(client.blob_fetches().await.is_empty(), "no fetch");
    assert!(!main.exists(), "the file must stay down");

    // And through the processor, written again by its author.
    file.put(&mut test, &author_kp).await?;
    assert!(
        !main.exists(),
        "the file must stay down when uploaded again"
    );

    file.del(&mut test, &author_kp).await?;
    cleanup_v1_user(&mut test, &author_kp, &author_id).await
}

/// A tag on a `{tsid}` address removes that legacy folder and its node, and leaves the owner's
/// v1 file alone: `H.png` stays servable until the moderator tags it too.
#[tokio_shared_rt::test(shared)]
async fn test_takedown_of_a_tsid_address_leaves_the_v1_folder_alone() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let moderator_kp = create_moderator(&mut test).await?;
    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:V1File:TsidTakedown", "takedown"),
        )
        .await?;
    let (file_id, v0_uri, v0_main) = create_v0_file(
        &mut test,
        &author_kp,
        &author_id,
        "Watcher:V1File:TsidTakedown",
    )
    .await?;
    set_user_epoch(&author_id, "v1").await?;
    let (v1_file, v1_uri, v1_main) = create_v1_file(
        &mut test,
        &author_kp,
        &author_id,
        "Watcher:V1File:TsidTakedown",
    )
    .await?;

    moderate(&mut test, &moderator_kp, &v0_uri).await?;

    assert!(
        !v0_main.exists(),
        "the takedown must remove the legacy folder"
    );
    assert!(
        !v0_file_node_exists(&author_id, &file_id).await?,
        "the takedown must delete the File node"
    );
    assert!(v1_main.exists(), "the v1 folder must be left alone");
    assert!(!is_taken_down(&v1_uri).await?);

    v1_file.del(&mut test, &author_kp).await?;
    cleanup_v1_user(&mut test, &author_kp, &author_id).await
}

/// A tag on a v1 file's address removes that folder and leaves the owner's legacy file alone:
/// its folder and its node stay.
#[tokio_shared_rt::test(shared)]
async fn test_takedown_of_a_v1_address_leaves_the_legacy_file_alone() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let moderator_kp = create_moderator(&mut test).await?;
    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:V1File:V1Takedown", "takedown"),
        )
        .await?;
    let (file_id, v0_uri, v0_main) = create_v0_file(
        &mut test,
        &author_kp,
        &author_id,
        "Watcher:V1File:V1Takedown",
    )
    .await?;
    set_user_epoch(&author_id, "v1").await?;
    let (_, v1_uri, v1_main) = create_v1_file(
        &mut test,
        &author_kp,
        &author_id,
        "Watcher:V1File:V1Takedown",
    )
    .await?;

    moderate(&mut test, &moderator_kp, &v1_uri).await?;

    assert!(!v1_main.exists(), "the takedown must remove the v1 folder");
    assert!(v0_main.exists(), "the legacy folder must be left alone");
    assert!(
        v0_file_node_exists(&author_id, &file_id).await?,
        "the File node must be left alone"
    );
    assert!(!is_taken_down(&v0_uri).await?);

    // Back to `v0`, so the gate lets the `pubky.app` cleanup through.
    set_user_epoch(&author_id, "v0").await?;
    let v0_path: ResourcePath = PubkyAppFile::create_path(&file_id).parse()?;
    test.cleanup_file(&author_kp, &v0_path).await?;
    test.cleanup_user(&author_kp).await
}
