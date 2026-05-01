use super::resource_utils::{compute_resource_id, find_resource_tag_for_app};
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::resource::tag::TagResource;
use nexus_common::models::tag::traits::TagCollection;
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

    // 2. User creates Universal Tags with the same deterministic tag_id in two app namespaces.
    let target_uri = "https://example.com/moderation-target";
    let label = "bitcoin";

    let tag = PubkyAppTag {
        uri: target_uri.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let tag_id = tag.create_id();
    let resource_id = compute_resource_id(target_uri);
    let mapky_path: ResourcePath = format!("/pub/mapky/tags/{tag_id}").parse()?;
    let eventky_path: ResourcePath = format!("/pub/eventky/tags/{tag_id}").parse()?;
    test.put(&user_kp, &mapky_path, &tag).await?;
    test.put(&user_kp, &eventky_path, &tag).await?;

    // 3. Confirm both Universal Tags exist in the graph
    let tag_result = find_resource_tag_for_app(&resource_id, label, "mapky").await?;
    assert!(
        tag_result.is_some(),
        "mapky Universal Tag should exist in graph after PUT"
    );
    let tag_result = find_resource_tag_for_app(&resource_id, label, "eventky").await?;
    assert!(
        tag_result.is_some(),
        "eventky Universal Tag should exist in graph after PUT"
    );

    // 4. Load the moderator key and create the moderator account
    let moderator_recovery_file =
        fs::read("./tests/event_processor/utils/moderator_key.pkarr").await?;
    let moderator_key =
        recovery_file::decrypt_recovery_file(&moderator_recovery_file, "password").unwrap();

    test.create_user(&moderator_key, &user).await?;

    // 5. Moderator places a standard pubky.app tag whose `uri` is the Universal Tag's own URI.
    //    The moderation system should recognise this as a Universal Tag and delete it.
    let universal_tag_uri = format!("pubky://{user_id}/pub/eventky/tags/{tag_id}");
    let moderation_tag = PubkyAppTag {
        uri: universal_tag_uri,
        label: "label_to_moderate".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let moderation_tag_path = moderation_tag.hs_path();
    test.put(&moderator_key, &moderation_tag_path, &moderation_tag)
        .await?;

    // 6. Confirm only the app-scoped Universal Tag has been deleted
    let tag_after = find_resource_tag_for_app(&resource_id, label, "eventky").await?;
    assert!(
        tag_after.is_none(),
        "eventky Universal Tag should be deleted after moderation"
    );
    let tag_after = find_resource_tag_for_app(&resource_id, label, "mapky").await?;
    assert!(
        tag_after.is_some(),
        "mapky Universal Tag should remain after eventky moderation"
    );

    // The global Resource tagger set is shared across apps, so the remaining mapky tag should
    // keep the user visible in cached tag details after eventky is removed.
    let cache_tags = TagResource::get_from_index(&resource_id, None, None, None, None, None, false)
        .await?
        .expect("global Resource tag cache should remain while mapky tag exists");
    assert_eq!(cache_tags.len(), 1);
    assert_eq!(cache_tags[0].label, label);
    assert_eq!(cache_tags[0].taggers, vec![user_id.to_string()]);

    Ok(())
}
