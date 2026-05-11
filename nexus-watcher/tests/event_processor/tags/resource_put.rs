use super::resource_utils::{check_resource_in_sorted_set, compute_resource_id, find_resource_tag};
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::resource::tag::TagResource;
use nexus_common::models::tag::search::TagSearch;
use nexus_common::models::tag::traits::TagCollection;
use nexus_common::types::Pagination;
use pubky::Keypair;
use pubky::ResourcePath;
use pubky_app_specs::traits::HashId;
use pubky_app_specs::{PubkyAppTag, PubkyAppUser};

/// Full cycle test: PUT a tag at an app-specific path (/pub/mapky/tags/TAG_ID)
/// and verify the Resource node is created in Neo4j with correct Redis indexes.
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_put_resource_tag_external_uri() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create a tagger user
    let user_kp = Keypair::random();
    let tagger = PubkyAppUser {
        bio: Some("test_resource_tag_external".to_string()),
        image: None,
        links: None,
        name: "Watcher:ResourceTag:External".to_string(),
        status: None,
    };
    let tagger_user_id = test.create_user(&user_kp, &tagger).await?;

    // Create a tag targeting an external URL
    let target_uri = "https://example.com/article?q=test";
    let label = "bitcoin";

    let tag = PubkyAppTag {
        uri: target_uri.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    // Compute tag_id and resource_id for assertions
    let tag_id = tag.create_id();
    let resource_id = compute_resource_id(target_uri);

    // Build custom path at /pub/mapky/tags/ instead of /pub/pubky.app/tags/
    let custom_path: ResourcePath = format!("/pub/mapky/tags/{tag_id}").parse()?;
    test.put(&user_kp, &custom_path, &tag).await?;

    // =============================================
    // LAYER 1: Verify Neo4j Graph
    // =============================================

    let tag_result = find_resource_tag(&resource_id, label)
        .await?
        .expect("Resource tag should exist in graph");

    assert_eq!(tag_result.label, label);
    assert_eq!(tag_result.app.as_deref(), Some("mapky"));
    assert_eq!(tag_result.tagger, tagger_user_id);
    assert_eq!(tag_result.scheme, "https");
    // URI should be normalized (lowercase host, trailing slash rules, etc.)
    assert_eq!(tag_result.uri, "https://example.com/article?q=test");

    // =============================================
    // LAYER 2: Verify Redis TagResource Cache
    // =============================================

    // TagResource sorted set should have label with score >= 1
    let cache_tags =
        TagResource::get_from_index(&resource_id, None, None, None, None, None, false).await?;

    assert!(cache_tags.is_some(), "TagResource cache should exist");
    let tag_details = cache_tags.unwrap();
    assert!(
        !tag_details.is_empty(),
        "Should have at least one tag label"
    );
    assert_eq!(tag_details[0].label, label);

    // =============================================
    // LAYER 3: Verify Redis ResourceStream Sorted Sets
    // =============================================

    // Global timeline
    let global_timeline =
        check_resource_in_sorted_set(&["Resources", "Global", "Timeline"], &resource_id).await?;
    assert!(
        global_timeline.is_some(),
        "Resource should be in global timeline"
    );

    // Per-app timeline
    let app_timeline =
        check_resource_in_sorted_set(&["Resources", "App", "mapky", "Timeline"], &resource_id)
            .await?;
    assert!(
        app_timeline.is_some(),
        "Resource should be in mapky app timeline"
    );

    // Per-tag timeline
    let tag_timeline =
        check_resource_in_sorted_set(&["Resources", "Tag", "bitcoin", "Timeline"], &resource_id)
            .await?;
    assert!(
        tag_timeline.is_some(),
        "Resource should be in bitcoin tag timeline"
    );

    // Combined app+tag timeline
    let app_tag_timeline = check_resource_in_sorted_set(
        &["Resources", "App", "mapky", "Tag", "bitcoin", "Timeline"],
        &resource_id,
    )
    .await?;
    assert!(
        app_tag_timeline.is_some(),
        "Resource should be in mapky+bitcoin combined timeline"
    );

    // Global tag search index should contain the label
    let tag_search = TagSearch::get_by_label(label, &Pagination::default()).await?;
    assert!(
        tag_search.is_some_and(|v| !v.is_empty()),
        "Label should exist in global tag search"
    );

    // Cleanup
    test.del(&user_kp, &custom_path).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}

/// Test tagging a pubky:// URI that is NOT a known Post/User (Internal-Unknown)
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_put_resource_tag_internal_unknown() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_resource_tag_internal_unknown".to_string()),
        image: None,
        links: None,
        name: "Watcher:ResourceTag:InternalUnknown".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    // Tag a pubky URI that's NOT pubky.app (eventky event)
    let target_uri = format!("pubky://{user_id}/pub/eventky.app/events/E001");
    let label = "conference";

    let tag = PubkyAppTag {
        uri: target_uri.clone(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let tag_id = tag.create_id();
    let resource_id = compute_resource_id(&target_uri);

    let custom_path: ResourcePath = format!("/pub/eventky.app/tags/{tag_id}").parse()?;
    test.put(&user_kp, &custom_path, &tag).await?;

    // Verify it was indexed as a Resource (not a Post or User)
    let tag_result = find_resource_tag(&resource_id, label)
        .await?
        .expect("Internal-unknown tag should be indexed as Resource");

    assert_eq!(tag_result.label, label);
    assert_eq!(tag_result.app.as_deref(), Some("eventky.app"));
    assert_eq!(tag_result.scheme, "pubky");

    // Cleanup
    test.del(&user_kp, &custom_path).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}
