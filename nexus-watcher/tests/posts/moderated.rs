use crate::{posts::utils::find_post_details, utils::watcher::WatcherTest};
use anyhow::Result;
use chrono::Utc;
use pubky::{recovery_file, Keypair};
use pubky_app_specs::{traits::HashId, PubkyAppPost, PubkyAppPostKind, PubkyAppTag, PubkyAppUser};
use tokio::fs;

#[tokio_shared_rt::test(shared)]
async fn test_moderated_post_lifecycle() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // 1. User signup and writes a post
    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_homeserver_post_to_moderate".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostModerate:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    let post = PubkyAppPost {
        content: "Watcher:PostModerate:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let post_id = test.create_post(&user_id, &post).await?;

    // 2. Confirm this post does exist
    let post_details = find_post_details(&user_id, &post_id).await.unwrap();
    assert_eq!(post_details.id, post_id);

    // 3. Load moderation service key and place a tag on that post with label "label_to_moderate"
    let moderator_recovery_file = fs::read("./tests/utils/moderator_key.pkarr").await.unwrap();
    let moderator_key =
        recovery_file::decrypt_recovery_file(&moderator_recovery_file, "password").unwrap();

    let moderator_id = test.create_user(&moderator_key, &user).await?;

    let tag = PubkyAppTag {
        uri: format!("pubky://{user_id}/pub/pubky.app/posts/{post_id}"),
        label: "label_to_moderate".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_url = format!(
        "pubky://{}/pub/pubky.app/tags/{}",
        moderator_id,
        tag.create_id()
    );
    // Put tag
    test.put(&tag_url, tag).await?;

    // 4. Confirm the post does not exist
    let post_details = find_post_details(&user_id, &post_id).await;
    assert!(post_details.is_err());

    Ok(())
}
