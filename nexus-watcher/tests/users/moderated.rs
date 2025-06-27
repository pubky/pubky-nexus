// tests/users/moderated.rs

use crate::{users::utils::find_user_details, utils::watcher::WatcherTest};
use anyhow::Result;
use chrono::Utc;
use pubky::{recovery_file, Keypair};
use pubky_app_specs::{traits::HashId, PubkyAppTag, PubkyAppUser};
use tokio::fs;

#[tokio_shared_rt::test(shared)]
async fn test_moderated_user_lifecycle() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // 1. Create the target user
    let user_key = Keypair::random();
    let target = PubkyAppUser {
        name: "Watcher:UserModerate:Target".to_string(),
        bio: Some("to be moderated".to_string()),
        image: None,
        links: None,
        status: None,
    };
    let target_id = test.create_user(&user_key, &target).await?;

    // 2. Confirm the user exists
    let details = find_user_details(&target_id).await?;
    assert_eq!(details.id.to_string(), target_id);

    // 3. Load moderator key and create moderator
    let mod_file = fs::read("./tests/utils/moderator_key.pkarr").await.unwrap();
    let mod_key = recovery_file::decrypt_recovery_file(&mod_file, "password").unwrap();
    let moderator_id = test.create_user(&mod_key, &target).await?;

    // 4. Tag the target user with the moderation label
    let tag = PubkyAppTag {
        uri: format!("pubky://{target_id}/pub/pubky.app/profile.json"),
        label: "label_to_moderate".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_url = format!(
        "pubky://{}/pub/pubky.app/tags/{}",
        moderator_id,
        tag.create_id()
    );
    test.put(&tag_url, tag.clone()).await?;

    // 5. Confirm the user no longer exists
    let details = find_user_details(&target_id).await;
    assert!(details.is_err());

    // 6. Confirm the user profile can be re-creating by pushing a new profile.json
    let new_profile = PubkyAppUser {
        name: "Watcher:UserModerate:Target".to_string(),
        bio: Some("i am back, will behave".to_string()),
        image: None,
        links: None,
        status: None,
    };
    let profile_url = format!("pubky://{target_id}/pub/pubky.app/profile.json");
    test.put(&profile_url, new_profile).await?;

    let details = find_user_details(&target_id).await?;
    assert_eq!(details.bio, Some("i am back, will behave".to_string()));

    // 7. User places a tag on himself (create at least 1 relationship)
    let self_tag = PubkyAppTag {
        uri: format!("pubky://{target_id}/pub/pubky.app/profile.json"),
        label: "tagging_myself".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let selftag_url = format!(
        "pubky://{}/pub/pubky.app/tags/{}",
        target_id,
        self_tag.create_id()
    );
    test.put(&selftag_url, self_tag).await?;

    // 8. Tag the target user with the moderation label
    test.put(&tag_url, tag).await?;

    // 9. Confirm the user does exist but the profile has been cleaned
    let details = find_user_details(&target_id).await?;
    assert_eq!(details.bio, None);

    Ok(())
}
