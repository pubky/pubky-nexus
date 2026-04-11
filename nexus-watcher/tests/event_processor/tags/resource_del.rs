use super::resource_utils::{
    check_resource_in_sorted_set, compute_resource_id, find_resource_tag, resource_exists_in_graph,
};
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::resource::tag::TagResource;
use nexus_common::models::tag::traits::TagCollection;
use pubky::Keypair;
use pubky::ResourcePath;
use pubky_app_specs::traits::HashId;
use pubky_app_specs::{PubkyAppTag, PubkyAppUser};

/// Test the full DEL cycle: PUT → DEL → verify Resource node is cleaned up
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_del_resource_tag() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_del_resource_tag".to_string()),
        image: None,
        links: None,
        name: "Watcher:ResourceTag:Del".to_string(),
        status: None,
    };
    let _user_id = test.create_user(&user_kp, &user).await?;

    let target_uri = "https://example.com/to-be-deleted";
    let label = "temporary";

    let tag = PubkyAppTag {
        uri: target_uri.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let tag_id = tag.create_id();
    let resource_id = compute_resource_id(target_uri);
    let custom_path: ResourcePath = format!("/pub/mapky/tags/{tag_id}").parse()?;

    // PUT the tag
    test.put(&user_kp, &custom_path, &tag).await?;

    // Verify it exists
    let tag_result = find_resource_tag(&resource_id, label).await?;
    assert!(tag_result.is_some(), "Tag should exist after PUT");

    // DEL the tag
    test.del(&user_kp, &custom_path).await?;

    // Verify the TAGGED relationship is removed
    let tag_after_del = find_resource_tag(&resource_id, label).await?;
    assert!(tag_after_del.is_none(), "Tag should be gone after DEL");

    // Verify the Resource node is cleaned up (orphan removal)
    let exists = resource_exists_in_graph(&resource_id).await?;
    assert!(
        !exists,
        "Resource node should be deleted when no tags remain"
    );

    // Verify Redis cache is cleaned up
    let cache_tags =
        TagResource::get_from_index(&resource_id, None, None, None, None, None, false).await?;
    // Should be None or empty
    let is_empty = cache_tags.is_none_or(|v| v.is_empty() || v[0].taggers_count == 0);
    assert!(is_empty, "TagResource cache should be empty after DEL");

    // Verify global taggers count decremented
    let global_count =
        check_resource_in_sorted_set(&["Resources", "Global", "TaggersCount"], &resource_id)
            .await?;
    let is_zero = global_count.is_none_or(|s| s <= 0);
    assert!(is_zero, "Global taggers count should be 0 after DEL");

    // Cleanup user
    test.cleanup_user(&user_kp).await?;

    Ok(())
}

/// Test PUT → DEL → re-PUT cycle (like post_put.rs unique count test)
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_resource_tag_put_del_reput() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_resource_tag_reput".to_string()),
        image: None,
        links: None,
        name: "Watcher:ResourceTag:RePut".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    let target_uri = "https://example.com/reput-test";
    let label = "reput-label";

    let tag = PubkyAppTag {
        uri: target_uri.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let tag_id = tag.create_id();
    let resource_id = compute_resource_id(target_uri);
    let custom_path: ResourcePath = format!("/pub/mapky/tags/{tag_id}").parse()?;

    // Step 1: PUT
    test.put(&user_kp, &custom_path, &tag).await?;
    let tag_result = find_resource_tag(&resource_id, label).await?;
    assert!(tag_result.is_some(), "Tag should exist after first PUT");

    // Step 2: DEL
    test.del(&user_kp, &custom_path).await?;
    let tag_result = find_resource_tag(&resource_id, label).await?;
    assert!(tag_result.is_none(), "Tag should be gone after DEL");

    // Step 3: Re-PUT
    test.put(&user_kp, &custom_path, &tag).await?;
    let tag_result = find_resource_tag(&resource_id, label)
        .await?
        .expect("Tag should exist again after re-PUT");
    assert_eq!(tag_result.tagger, user_id);
    assert_eq!(tag_result.app.as_deref(), Some("mapky"));

    // Cleanup
    test.del(&user_kp, &custom_path).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}
