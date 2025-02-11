use crate::{utils::TestServiceServer, watcher::utils::watcher::WatcherTest};
use anyhow::Result;
use chrono::Utc;
use pubky_app_specs::traits::HashId;
use pubky_app_specs::{PubkyAppBlob, PubkyAppFile, PubkyAppUser};
use pubky_common::crypto::Keypair;
use pubky_nexus::PubkyConnector;

/// We'll reuse your existing macros and test environment
#[tokio_shared_rt::test(shared)]
async fn test_user_avatar_endpoint_png() -> Result<()> {
    // 1. Boot the watchers, servers, etc.
    let mut test = WatcherTest::setup().await?;
    TestServiceServer::get_test_server().await;

    // 2. Create a fresh user
    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("I have a custom PNG avatar".to_string()),
        image: None,
        links: None,
        name: "PNGAvatarUser".to_string(),
        status: Some("PNG Testing".to_string()),
    };
    let user_id = test.create_user(&keypair, &user).await?;

    // 3. Read the local `avatar.png` file contents
    //    (assuming it's small enough for test usage)
    let image_bytes = tokio::fs::read("tests/watcher/users/avatar.png").await?;
    let image_size = image_bytes.len();

    // 4. Publish that byte blob to Pubky
    let blob = PubkyAppBlob::new(image_bytes.clone());
    let blob_url = format!(
        "pubky://{}/pub/pubky.app/blobs/{}",
        user_id,
        blob.create_id()
    );
    let pubky_client = PubkyConnector::get_pubky_client()?;
    pubky_client.put(&blob_url).body(blob.0).send().await?;

    // 5. Create a new file referencing that blob
    let file = PubkyAppFile {
        name: "avatar.png".to_string(),
        content_type: "image/png".to_string(),
        src: blob_url.clone(),
        size: image_size as i64,
        created_at: Utc::now().timestamp_millis(),
    };
    let (file_id, _event_id) = test.create_file(&user_id, &file).await?;

    // 6. Update the user so that its `image` field references
    let updated_user = PubkyAppUser {
        bio: user.bio.clone(),
        links: user.links.clone(),
        name: user.name.clone(),
        status: user.status.clone(),
        image: Some(format!("pubky://{user_id}/pub/pubky.app/files/{file_id}")),
    };
    test.create_user(&keypair, &updated_user).await?;

    // 7. Issue a GET to the new `/avatar` route using your test client
    let client = httpc_test::new_client(crate::service::utils::host_url().await)?;
    let url_path = format!("/v0/user/{user_id}/avatar");
    let response = client.do_get(&url_path).await?;

    // 8. Verify response
    assert_eq!(
        response.status(),
        200,
        "Should receive 200 OK serving the avatar"
    );

    // Check Content-Type
    let ctype = response
        .header("content-type")
        .unwrap_or("none".to_string());
    assert_eq!(ctype, "image/png", "Expected `content-type: image/png`");

    // Compare lengths
    let clength = response.header("content-length").unwrap_or("0".to_string());
    assert_eq!(
        clength,
        format!("{}", image_size),
        "Returned PNG avatar size differs from what we uploaded"
    );

    // 10. Done!
    Ok(())
}
