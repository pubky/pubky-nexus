use super::resource_utils::{compute_resource_id, find_resource_tag};
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use pubky::ResourcePath;
use pubky::{recovery_file, Keypair};
use pubky_app_specs::traits::HashId;
use pubky_app_specs::{PubkyAppTag, PubkyAppUser};
use tokio::fs;

/// Verify that a moderation tag whose `uri` field points to a Universal Tag URI
/// (i.e. a tag stored at a non-pubky.app path such as `/pub/mapky/tags/<tag_id>`)
/// causes that Universal Tag to be deleted.
#[tokio_shared_rt::test(shared)]
async fn test_moderated_universal_tag_lifecycle() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // 1. Create the user who will publish the Universal Tag
    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_moderated_universal_tag".to_string()),
        image: None,
        links: None,
        name: "Watcher:UniversalTagModerate:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    // 2. User creates a Universal Tag at /pub/mapky/tags/<tag_id> pointing to an external URI
    let target_uri = "https://example.com/moderation-target";
    let label = "bitcoin";

    let tag = PubkyAppTag {
        uri: target_uri.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let tag_id = tag.create_id();
    let resource_id = compute_resource_id(target_uri);
    let custom_path: ResourcePath = format!("/pub/mapky/tags/{tag_id}").parse()?;
    test.put(&user_kp, &custom_path, &tag).await?;

    // 3. Confirm the Universal Tag exists in the graph
    let tag_result = find_resource_tag(&resource_id, label).await?;
    assert!(
        tag_result.is_some(),
        "Universal Tag should exist in graph after PUT"
    );

    // 4. Load the moderator key and create the moderator account
    let moderator_recovery_file =
        fs::read("./tests/event_processor/utils/moderator_key.pkarr").await?;
    let moderator_key =
        recovery_file::decrypt_recovery_file(&moderator_recovery_file, "password").unwrap();

    test.create_user(&moderator_key, &user).await?;

    // 5. Moderator places a standard pubky.app tag whose `uri` is the Universal Tag's own URI.
    //    The moderation system should recognise this as a Universal Tag and delete it.
    let universal_tag_uri = format!("pubky://{user_id}/pub/mapky/tags/{tag_id}");
    let moderation_tag = PubkyAppTag {
        uri: universal_tag_uri,
        label: "label_to_moderate".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let moderation_tag_path = moderation_tag.hs_path();
    test.put(&moderator_key, &moderation_tag_path, &moderation_tag)
        .await?;

    // 6. Confirm the Universal Tag has been deleted
    let tag_after = find_resource_tag(&resource_id, label).await?;
    assert!(
        tag_after.is_none(),
        "Universal Tag should be deleted after moderation"
    );

    Ok(())
}
