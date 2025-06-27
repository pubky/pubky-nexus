use std::path::Path;

use crate::posts::utils::find_post_details;
use crate::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::{file::FileDetails, traits::Collection};
use pubky::Keypair;
use pubky_app_specs::{
    traits::{HasIdPath, HashId},
    PubkyAppBlob, PubkyAppFile, PubkyAppPost, PubkyAppPostKind, PubkyAppUser,
};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_del_post_with_attachments() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_homeserver_del_post_with_attachments".to_string()),
        image: None,
        links: None,
        name: "Watcher:DelWithAttachmentEvent:User".to_string(),
        status: None,
    };

    let user_id = test.create_user(&keypair, &user).await?;

    let mut file_urls = Vec::new();
    let mut file_ids = Vec::new();

    for i in 0..2 {
        let blob_data = format!("DEL me, im part of attachment of file {}", i + 1);
        let blob = PubkyAppBlob::new(blob_data.as_bytes().to_vec());
        let blob_id = blob.create_id();
        let blob_url = format!("pubky://{}{}", user_id, blob.create_path(&blob_id));

        test.create_file_from_body(blob_url.as_str(), blob.0.clone())
            .await?;
        test.ensure_event_processing_complete().await?;

        let file = PubkyAppFile {
            name: format!("post_attachment_DEL-{i}"),
            content_type: "text/plain".to_string(),
            src: blob_url.clone(),
            size: blob.0.len(),
            created_at: Utc::now().timestamp_millis(),
        };
        let (file_id, file_url) = test.create_file(&user_id, &file).await?;
        file_urls.push(file_url);
        file_ids.push(file_id);
    }

    let post = PubkyAppPost {
        content: "Watcher:DelWithAttachmentEvent:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: Some(file_urls.clone()),
    };

    let post_id = test.create_post(&user_id, &post).await?;

    let post_details = find_post_details(&user_id, &post_id).await.unwrap();

    assert_eq!(post_details.id, post_id);
    assert_eq!(post_details.content, post.content);
    assert_eq!(post_details.attachments, Some(file_urls));

    // Cleanup
    test.cleanup_post(&user_id, &post_id).await?;
    // If the post has attachments, it also needs to send DEL event
    test.cleanup_file(&user_id, &file_ids[0]).await?;
    test.cleanup_file(&user_id, &file_ids[1]).await?;

    for i in 0..2 {
        let files = FileDetails::get_by_ids(
            vec![vec![user_id.as_str(), file_ids[i].as_str()].as_slice()].as_slice(),
        )
        .await
        .expect("Failed to fetch files from Nexus");

        let result_file = files[0].as_ref();
        assert!(result_file.is_none());

        // Assert: Ensure it's deleted
        let blob_static_path = format!("./static/files/{}/{}/main", &user_id, &file_ids[i]);
        assert!(
            !Path::new(&blob_static_path).exists(),
            "File cannot exist after DEL event"
        );
    }

    Ok(())
}
