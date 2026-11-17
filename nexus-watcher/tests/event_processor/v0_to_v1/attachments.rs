//! The attachments step: a migrated user's attachments name the files the migrator wrote, and
//! a replayed flip batch stays silent for a short post with attachments.

use crate::event_processor::posts::utils::{
    find_post_details, long_post, pubky_id, short_post, short_reply, test_user, v1_test_user,
};
use crate::event_processor::users::utils::{assert_flipped, set_user_epoch};
use crate::event_processor::utils::watcher::{retrieve_and_handle_event_line, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::notification::Notification;
use nexus_common::models::post::PostView;
use nexus_common::types::Pagination;
use nexus_watcher::events::v0_to_v1;
use pubky::{Keypair, ResourcePath};
use pubky_social_specs::legacy_v0::traits::{HasIdPath, HashId};
use pubky_social_specs::legacy_v0::{
    blob_uri_builder, file_uri_builder as v0_file_uri_builder,
    post_uri_builder as v0_post_uri_builder, user_uri_builder as v0_user_uri_builder, PubkyAppBlob,
    PubkyAppFile, PubkyAppPost,
};
use pubky_social_specs::{
    file_uri_builder as v1_file_uri_builder, user_uri_builder as v1_user_uri_builder,
    PubkySocialAttachment, PubkySocialPost, PubkySocialPostKind,
};

/// Uploads `bytes` as `user_id`'s v0 blob and returns its id and address.
pub(super) async fn upload_blob(
    test: &mut WatcherTest,
    user_kp: &Keypair,
    user_id: &str,
    bytes: &str,
) -> Result<(String, String)> {
    let blob = PubkyAppBlob::new(bytes.as_bytes().to_vec());
    let blob_id = blob.create_id();
    test.create_file_from_body(user_kp, &PubkyAppBlob::create_path(&blob_id), blob.0)
        .await?;
    Ok((
        blob_id.clone(),
        blob_uri_builder(user_id.to_string(), blob_id),
    ))
}

/// Writes a v0 `File` object of `user_id` naming the blob at `src`, and returns its id, its
/// path and its address.
pub(super) async fn create_file(
    test: &mut WatcherTest,
    user_kp: &Keypair,
    user_id: &str,
    name: &str,
    src: &str,
    content_type: &str,
) -> Result<(String, ResourcePath, String)> {
    let file = PubkyAppFile {
        name: name.to_string(),
        content_type: content_type.to_string(),
        src: src.to_string(),
        size: 32,
        created_at: Utc::now().timestamp_millis(),
    };
    let (file_id, file_path) = test.create_file(user_kp, &file).await?;
    let uri = v0_file_uri_builder(user_id.to_string(), file_id.clone());
    Ok((file_id, file_path, uri))
}

/// The migrated address of `user_id`'s blob `hash` under `ext`.
pub(super) fn v1_file_uri(user_id: &str, hash: &str, ext: &str) -> String {
    v1_file_uri_builder(user_id.to_string(), format!("{hash}.{ext}"))
}

/// A short post with `content` and `attachments`.
fn short_post_with(content: &str, attachments: Vec<String>) -> PubkyAppPost {
    PubkyAppPost {
        attachments: Some(attachments),
        ..short_post(content)
    }
}

/// The attachments the API serves for a post and the ones its graph node holds, which the
/// migration keeps equal.
async fn served_and_stored_attachments(
    author_id: &str,
    post_id: &str,
) -> Result<(Vec<String>, Vec<String>)> {
    let served = PostView::get_by_id(author_id, post_id, None, None, None)
        .await?
        .expect("the post is served");
    let stored = find_post_details(author_id, post_id).await?;
    Ok((served.details.attachments, stored.attachments))
}

/// The flip rewrites every attachment in order, each as the migrator does: an own file in the
/// map and an own blob to `social/v1/files/{hash}.{ext}`, two uploads of one blob to one
/// address under the lowest `tsid`'s extension, an own blob no file names under `bin`, a
/// profile to its v1 URI; another user's file, a file whose `src` is not the owner's blob,
/// another app's path and a web URL keep their address. An article keeps its image first
/// attachment, rewritten, and its content. Running the migration again changes nothing.
#[tokio_shared_rt::test(shared)]
async fn test_flip_rewrites_attachments_to_the_migrated_file_addresses() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:Migration:Attachments:Author", "migration"),
        )
        .await?;
    let other_kp = Keypair::random();
    let other_id = test
        .create_user(
            &other_kp,
            &test_user("Watcher:Migration:Attachments:Other", "migration"),
        )
        .await?;

    let unique = format!("{author_id}:{}", Utc::now().timestamp_micros());
    // One blob named by two files: the lower `tsid` is the image
    let (shared_hash, shared_src) = upload_blob(
        &mut test,
        &author_kp,
        &author_id,
        &format!("shared:{unique}"),
    )
    .await?;
    let (image_id, image_path, image_uri) = create_file(
        &mut test,
        &author_kp,
        &author_id,
        "shared.png",
        &shared_src,
        "image/png",
    )
    .await?;
    let (text_id, text_path, text_uri) = create_file(
        &mut test,
        &author_kp,
        &author_id,
        "shared.txt",
        &shared_src,
        "text/plain",
    )
    .await?;
    assert!(image_id < text_id, "the image is the lowest tsid");
    // A blob no file names
    let (orphan_hash, orphan_src) = upload_blob(
        &mut test,
        &author_kp,
        &author_id,
        &format!("orphan:{unique}"),
    )
    .await?;
    // Another user's file, and an own file whose `src` is that user's blob
    let (_, other_src) =
        upload_blob(&mut test, &other_kp, &other_id, &format!("other:{unique}")).await?;
    let (_, other_file_path, other_file_uri) = create_file(
        &mut test,
        &other_kp,
        &other_id,
        "other.png",
        &other_src,
        "image/png",
    )
    .await?;
    let (_, foreign_path, foreign_uri) = create_file(
        &mut test,
        &author_kp,
        &author_id,
        "foreign.png",
        &other_src,
        "image/png",
    )
    .await?;
    let other_app_uri = format!("pubky://{author_id}/pub/other.app/items/1");
    let web_uri = "https://example.com/a.png".to_string();

    let attachments = vec![
        image_uri.clone(),
        text_uri.clone(),
        shared_src.clone(),
        orphan_src.clone(),
        other_file_uri.clone(),
        foreign_uri.clone(),
        other_app_uri.clone(),
        web_uri.clone(),
        v0_user_uri_builder(other_id.clone()),
    ];
    let (post_id, post_path) = test
        .create_post(
            &author_kp,
            &short_post_with("Watcher:Migration:Attachments:Post", attachments.clone()),
        )
        .await?;
    let (article_id, article_path) = test
        .create_post(
            &author_kp,
            &PubkyAppPost {
                attachments: Some(vec![image_uri.clone()]),
                ..long_post("Watcher:Migration:Attachments:Article")
            },
        )
        .await?;
    let (bare_id, bare_path) = test
        .create_post(
            &author_kp,
            &short_post("Watcher:Migration:Attachments:Bare"),
        )
        .await?;
    // Served once before the flip, so the caches hold the old addresses
    let (served, stored) = served_and_stored_attachments(&author_id, &post_id).await?;
    assert_eq!(served, attachments);
    assert_eq!(stored, attachments);

    let profile_path = test
        .put_v1_profile(
            &author_kp,
            &v1_test_user("Watcher:Migration:Attachments:Author:V1", "migration"),
        )
        .await?;
    assert_flipped(&author_id).await;

    let shared_v1 = v1_file_uri(&author_id, &shared_hash, "png");
    assert_eq!(
        shared_v1,
        format!("pubky://{author_id}/pub/social/v1/files/{shared_hash}.png")
    );
    let expected = vec![
        shared_v1.clone(),
        shared_v1.clone(),
        shared_v1.clone(),
        v1_file_uri(&author_id, &orphan_hash, "bin"),
        other_file_uri.clone(),
        foreign_uri.clone(),
        other_app_uri.clone(),
        web_uri.clone(),
        v1_user_uri_builder(other_id.clone()),
    ];
    let (served, stored) = served_and_stored_attachments(&author_id, &post_id).await?;
    assert_eq!(served, expected);
    assert_eq!(stored, expected);

    let article = PostView::get_by_id(&author_id, &article_id, None, None, None)
        .await?
        .expect("the article is served");
    assert_eq!(article.details.attachments, vec![shared_v1.clone()]);
    assert_eq!(
        article.details.content,
        "Watcher:Migration:Attachments:Article"
    );
    let (served, stored) = served_and_stored_attachments(&author_id, &bare_id).await?;
    assert!(served.is_empty());
    assert!(stored.is_empty());

    // Rerun: the same attachments
    let author = pubky_id(&author_id)?;
    let files = v0_to_v1::files::FileMap::load(&author).await?;
    v0_to_v1::posts::migrate(&author, &files, 2).await?;
    let (served, stored) = served_and_stored_attachments(&author_id, &post_id).await?;
    assert_eq!(served, expected);
    assert_eq!(stored, expected);
    let (served, _) = served_and_stored_attachments(&author_id, &article_id).await?;
    assert_eq!(served, vec![shared_v1.clone()]);

    // Back to `v0`, so the gate lets the `pubky.app` cleanup through; the v1 DEL is then ignored
    set_user_epoch(&author_id, "v0").await?;
    test.del(&author_kp, &profile_path).await?;
    for path in [&post_path, &article_path, &bare_path] {
        test.cleanup_post(&author_kp, path).await?;
    }
    for path in [&image_path, &text_path, &foreign_path] {
        test.cleanup_file(&author_kp, path).await?;
    }
    test.cleanup_file(&other_kp, &other_file_path).await?;
    test.cleanup_user(&other_kp).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}

/// A batch spanning the flip, read again: the migrator's copy of a short post with attachments
/// was ignored before the flip and is read again after it. Its attachments now equal the
/// migrated ones, so the replay is no edit and whoever replied hears nothing.
#[tokio_shared_rt::test(shared)]
async fn test_replayed_flip_batch_sends_no_edit_notification_for_a_short_post_with_attachments(
) -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:Migration:Replay:Author", "migration"),
        )
        .await?;
    let replier_kp = Keypair::random();
    let replier_id = test
        .create_user(
            &replier_kp,
            &test_user("Watcher:Migration:Replay:Replier", "migration"),
        )
        .await?;

    let unique = format!("{author_id}:{}", Utc::now().timestamp_micros());
    let (hash, src) = upload_blob(
        &mut test,
        &author_kp,
        &author_id,
        &format!("replay:{unique}"),
    )
    .await?;
    let (_, file_path, file_uri) = create_file(
        &mut test,
        &author_kp,
        &author_id,
        "replay.png",
        &src,
        "image/png",
    )
    .await?;

    let content = "Watcher:Migration:Replay:Post";
    let (post_id, post_path) = test
        .create_post(
            &author_kp,
            &short_post_with(content, vec![file_uri.clone()]),
        )
        .await?;
    let (_, reply_path) = test
        .create_post(
            &replier_kp,
            &short_reply(
                "Watcher:Migration:Replay:Reply",
                v0_post_uri_builder(author_id.clone(), post_id.clone()),
            ),
        )
        .await?;
    let before = find_post_details(&author_id, &post_id).await?;

    // The migrator's copy, written before the profile: ignored, the user is still `v0`
    let migrated_file = v1_file_uri(&author_id, &hash, "png");
    let copy = PubkySocialPost::new(
        content.to_string(),
        PubkySocialPostKind::Note,
        None,
        None,
        vec![PubkySocialAttachment::new(
            migrated_file.clone(),
            None,
            Some("replay.png".to_string()),
        )],
    );
    let copy_path = test
        .put_v1_post_version(&author_kp, &post_id, &post_id, &copy)
        .await?;
    assert_eq!(
        find_post_details(&author_id, &post_id).await?.attachments,
        vec![file_uri.clone()]
    );

    // The flip
    let profile_path = test
        .put_v1_profile(
            &author_kp,
            &v1_test_user("Watcher:Migration:Replay:Author:V1", "migration"),
        )
        .await?;
    assert_flipped(&author_id).await;
    let (served, stored) = served_and_stored_attachments(&author_id, &post_id).await?;
    assert_eq!(served, vec![migrated_file.clone()]);
    assert_eq!(stored, vec![migrated_file.clone()]);

    // The batch read again: the copy, now of a `v1` user
    let line = format!("PUT pubky://{author_id}{copy_path}");
    let event_handler = test.event_processor_runner.event_handler.clone();
    retrieve_and_handle_event_line(&line, event_handler).await?;

    let notifications = Notification::get_by_id(&replier_id, Pagination::default()).await?;
    assert!(
        notifications.is_empty(),
        "a replayed flip batch must not read as an edit, got {notifications:?}"
    );
    let after = find_post_details(&author_id, &post_id).await?;
    assert_eq!(after.content, content);
    assert_eq!(after.attachments, vec![migrated_file.clone()]);
    assert_eq!(after.indexed_at, before.indexed_at);
    let (served, _) = served_and_stored_attachments(&author_id, &post_id).await?;
    assert_eq!(served, vec![migrated_file]);

    // Back to `v0`, so the gate lets the `pubky.app` cleanup through; the v1 DEL is then ignored
    set_user_epoch(&author_id, "v0").await?;
    test.del(&author_kp, &copy_path).await?;
    test.del(&author_kp, &profile_path).await?;
    test.cleanup_post(&replier_kp, &reply_path).await?;
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_file(&author_kp, &file_path).await?;
    test.cleanup_user(&replier_kp).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}
