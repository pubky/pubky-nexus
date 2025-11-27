use super::utils::find_post_details;
use crate::event_processor::utils::watcher::{assert_file_details, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use pubky::Keypair;
use pubky_app_specs::{
    blob_uri_builder,
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
    let (file_id, file_path) = test.create_file(&user_kp, &file).await?;

    assert_file_details(&user_id, &file_id, &blob_absolute_url, &file).await;

    let post_attachments = Some(vec![file_path.to_string()]);
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
