use super::utils::find_post_details;
use crate::event_processor::utils::watcher::{assert_file_details, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use pubky::Keypair;
use pubky_app_specs::{
    blob_uri_builder, file_uri_builder,
    traits::{HasIdPath, HashId},
    PubkyAppBlob, PubkyAppFile, PubkyAppPost, PubkyAppPostKind, PubkyAppUser,
};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_attachments() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_homeserver_post_event".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostEvent:User".to_string(),
        status: None,
    };

    let user_id = test.create_user(&user_kp, &user).await?;

    let blob_data = "Hello World!".to_string();
    let blob = PubkyAppBlob::new(blob_data.as_bytes().to_vec());
    let blob_id = blob.create_id();
    let blob_relative_url = PubkyAppBlob::create_path(&blob_id);
    let blob_absolute_url = blob_uri_builder(user_id.clone(), blob_id);

    test.create_file_from_body(&user_kp, blob_relative_url.as_str(), blob.0.clone())
        .await?;
    test.ensure_event_processing_complete().await?;

    let file = PubkyAppFile {
        name: "attachment".to_string(),
        content_type: "text/plain".to_string(),
        src: blob_absolute_url.clone(),
        size: blob.0.len(),
        created_at: Utc::now().timestamp_millis(),
    };
    let (file_id, _file_path) = test.create_file(&user_kp, &file).await?;

    assert_file_details(&user_id, &file_id, &blob_absolute_url, &file).await;

    let post_attachments = Some(vec![file_uri_builder(user_id.clone(), file_id.clone())]);
    let post = PubkyAppPost {
        content: "Watcher:PostEvent:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: post_attachments.clone(),
    };

    let (post_id, post_path) = test.create_post(&user_kp, &post).await?;

    let post_details = find_post_details(&user_id, &post_id).await.unwrap();

    assert_eq!(post_details.id, post_id);
    assert_eq!(post_details.content, post.content);
    assert_eq!(post_details.attachments, post_attachments);
    // Cleanup
    test.cleanup_user(&user_kp).await?;
    test.cleanup_post(&user_kp, &post_path).await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_attachment_only_edits() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_homeserver_post_attachment_only_edits".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostAttachmentOnlyEdits:User".to_string(),
        status: None,
    };

    let user_id = test.create_user(&user_kp, &user).await?;

    let blob_a = PubkyAppBlob::new("First attachment".as_bytes().to_vec());
    let first_blob_id = blob_a.create_id();
    let blob_a_relative_url = PubkyAppBlob::create_path(&first_blob_id);
    let blob_a_absolute_url = blob_uri_builder(user_id.clone(), first_blob_id);

    test.create_file_from_body(&user_kp, blob_a_relative_url.as_str(), blob_a.0.clone())
        .await?;

    let file_a = PubkyAppFile {
        name: "attachment-1".to_string(),
        content_type: "text/plain".to_string(),
        src: blob_a_absolute_url.clone(),
        size: blob_a.0.len(),
        created_at: Utc::now().timestamp_millis(),
    };
    let (first_file_id, first_file_path) = test.create_file(&user_kp, &file_a).await?;
    let attachment_a = file_uri_builder(user_id.clone(), first_file_id.clone());

    let blob_b = PubkyAppBlob::new("Second attachment".as_bytes().to_vec());
    let blob_b_id = blob_b.create_id();
    let blob_b_relative_url = PubkyAppBlob::create_path(&blob_b_id);
    let blob_b_absolute_url = blob_uri_builder(user_id.clone(), blob_b_id);

    test.create_file_from_body(&user_kp, blob_b_relative_url.as_str(), blob_b.0.clone())
        .await?;

    let file_b = PubkyAppFile {
        name: "attachment-2".to_string(),
        content_type: "text/plain".to_string(),
        src: blob_b_absolute_url.clone(),
        size: blob_b.0.len(),
        created_at: Utc::now().timestamp_millis(),
    };
    let (file_b_id, file_b_path) = test.create_file(&user_kp, &file_b).await?;
    let attachment_b = file_uri_builder(user_id.clone(), file_b_id.clone());

    let blob_c = PubkyAppBlob::new("Third attachment".as_bytes().to_vec());
    let blob_c_id = blob_c.create_id();
    let blob_c_relative_url = PubkyAppBlob::create_path(&blob_c_id);
    let blob_c_absolute_url = blob_uri_builder(user_id.clone(), blob_c_id);

    test.create_file_from_body(&user_kp, blob_c_relative_url.as_str(), blob_c.0.clone())
        .await?;

    let third_file = PubkyAppFile {
        name: "attachment-3".to_string(),
        content_type: "text/plain".to_string(),
        src: blob_c_absolute_url.clone(),
        size: blob_c.0.len(),
        created_at: Utc::now().timestamp_millis(),
    };
    let (file_c_id, file_c_path) = test.create_file(&user_kp, &third_file).await?;
    let attachment_c = file_uri_builder(user_id.clone(), file_c_id.clone());

    let mut post = PubkyAppPost {
        content: "Watcher:PostAttachmentOnlyEdits:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: Some(vec![attachment_a.clone(), attachment_b.clone()]),
    };

    let (post_id, post_path) = test.create_post(&user_kp, &post).await?;

    // Check initial post state
    let post_details = find_post_details(&user_id, &post_id).await?;
    assert_eq!(
        post_details.attachments,
        Some(vec![attachment_a.clone(), attachment_b.clone()])
    );

    post.attachments = Some(vec![attachment_b.clone(), attachment_c.clone()]);
    test.put(&user_kp, &post_path, &post).await?;

    // Check post after attachments were changed
    let post_details = find_post_details(&user_id, &post_id).await?;
    assert_eq!(post_details.content, post.content);
    assert_eq!(
        post_details.attachments,
        Some(vec![attachment_b.clone(), attachment_c.clone()])
    );

    post.attachments = Some(vec![attachment_c.clone()]);
    test.put(&user_kp, &post_path, &post).await?;

    // Check post after attachments were removed
    let post_details = find_post_details(&user_id, &post_id).await?;
    assert_eq!(post_details.content, post.content);
    assert_eq!(
        post_details.attachments,
        Some(vec![attachment_c]),
        "Expected the removed attachment to no longer appear in post lookup"
    );

    test.cleanup_post(&user_kp, &post_path).await?;
    test.cleanup_file(&user_kp, &first_file_path).await?;
    test.cleanup_file(&user_kp, &file_b_path).await?;
    test.cleanup_file(&user_kp, &file_c_path).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}
