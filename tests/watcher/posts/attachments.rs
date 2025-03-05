use crate::watcher::posts::utils::find_post_details;
use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use pubky::Keypair;
use pubky_app_specs::{
    traits::HasPath, PubkyAppBlob, PubkyAppFile, PubkyAppPost, PubkyAppPostKind, PubkyAppUser,
};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_attachments() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_homeserver_post_event".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostEvent:User".to_string(),
        status: None,
    };

    let user_id = test.create_user(&keypair, &user).await?;

    let blob_data = "Hello World!".to_string();
    let blob = PubkyAppBlob::new(blob_data.as_bytes().to_vec());
    let blob_url = format!("pubky://{}{}", user_id, blob.create_path());

    test.create_file_from_body(blob_url.as_str(), blob.0.clone()).await?;
    test.ensure_event_processing_complete().await?;

    let file = PubkyAppFile {
        name: "attachment".to_string(),
        content_type: "text/plain".to_string(),
        src: blob_url.clone(),
        size: blob.0.len() as i64,
        created_at: Utc::now().timestamp_millis(),
    };
    let (_, file_url) = test.create_file(&user_id, &file).await?;

    let post = PubkyAppPost {
        content: "Watcher:PostEvent:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: Some(vec![file_url.clone()]),
    };

    let post_id = test.create_post(&user_id, &post).await?;

    let post_details = find_post_details(&user_id, &post_id).await.unwrap();

    assert_eq!(post_details.id, post_id);
    assert_eq!(post_details.content, post.content);
    assert_eq!(post_details.attachments, Some(vec![file_url]));
    // Cleanup
    test.cleanup_user(&user_id).await?;
    test.cleanup_post(&user_id, &post_id).await?;

    Ok(())
}
