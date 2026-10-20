use super::resource_utils::{compute_resource_id, find_resource_tag};
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::resource::view::ResourceView;
use nexus_common::models::tag::search::TagSearch;
use nexus_common::types::Pagination;
use pubky::Keypair;
use pubky::ResourcePath;
use pubky_app_specs::traits::HashId;
use pubky_app_specs::{PubkyAppTag, PubkyAppUser};

/// Full cycle test: PUT a tag at an app-specific path (/pub/mapky/tags/TAG_ID)
/// and verify the Resource node is created in Neo4j, with its label in the tag
/// search index.
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_put_resource_tag_external_uri() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

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

    // Create a tag targeting an external URL. The label is unique to this run,
    // so only this put can add it to the tag search index
    let target_uri = "https://example.com/article?q=test";
    let label = format!("resput{}", Utc::now().timestamp_millis() % 100_000_000);
    let label = label.as_str();

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
    // LAYER 2: Verify the tag list and the tag search index
    // =============================================

    // The tag list is read from the graph
    let tag_details = ResourceView::get_by_id(&resource_id, None, None, None, None)
        .await?
        .expect("Resource should have a tag list")
        .tags;
    assert_eq!(tag_details.len(), 1, "Should have one tag label");
    assert_eq!(tag_details[0].label, label);
    assert_eq!(tag_details[0].taggers, [tagger_user_id.as_str()]);

    // Global tag search index should contain the label
    let tag_search = TagSearch::get_by_label(label, &Pagination::default()).await?;
    assert!(
        tag_search.is_some_and(|v| !v.is_empty()),
        "Label should exist in global tag search"
    );

    // Cleanup: resource tag deletes leave the label in tag search
    test.del(&user_kp, &custom_path).await?;
    TagSearch::del_from_index(label).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}

/// A retried put finds the edge already written (`Updated`) and adds the label to
/// the tag search index again, so a lost index write heals.
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_resource_tag_retry_heals_search_index() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_resource_tag_retry_heals_search_index".to_string()),
        image: None,
        links: None,
        name: "Watcher:ResourceTag:RetryHeal".to_string(),
        status: None,
    };
    test.create_user(&user_kp, &user).await?;

    // A label unique to this run, so no other tag keeps it in the search index
    let label = format!("resretry{}", Utc::now().timestamp_millis() % 100_000_000);
    let tag = PubkyAppTag {
        uri: "https://example.com/retry-heal".to_string(),
        label: label.clone(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path: ResourcePath = format!("/pub/mapky/tags/{}", tag.create_id()).parse()?;
    test.put(&user_kp, &tag_path, &tag).await?;

    // Simulate the partial failure a retry recovers from: the edge exists, but
    // the search index write was lost
    TagSearch::del_from_index(&label).await?;

    // Re-put the same tag: the id hashes (uri, label), so it lands on the same
    // path and the graph reports the edge as Updated
    let retry_tag = PubkyAppTag {
        created_at: Utc::now().timestamp_millis(),
        ..tag
    };
    test.put(&user_kp, &tag_path, &retry_tag).await?;

    let tag_search = TagSearch::get_by_label(&label, &Pagination::default()).await?;
    assert!(
        tag_search.is_some_and(|v| !v.is_empty()),
        "The retry should add the label to tag search again"
    );

    // Resource tag deletes leave the label in tag search, so remove it here
    test.del(&user_kp, &tag_path).await?;
    TagSearch::del_from_index(&label).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}

/// Test tagging a pubky:// URI that is NOT a known Post/User (Internal-Unknown)
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_put_resource_tag_internal_unknown() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

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
