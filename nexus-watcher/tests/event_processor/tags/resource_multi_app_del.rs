use super::resource_utils::{compute_resource_id, count_resource_tags, find_resource_tag_by_app};
use super::utils::find_user_tag;
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::resource::tag::TagResource;
use nexus_common::models::tag::traits::TagCollection;
use pubky::Keypair;
use pubky::ResourcePath;
use pubky_app_specs::traits::HashId;
use pubky_app_specs::{PubkyAppTag, PubkyAppUser};

/// Test: same label across different app namespaces — deletion of each tag is
/// correctly scoped to its own app namespace.
///
/// Scenario
/// --------
/// A single user creates three tags that all share the same `label`:
///   1. A standard pubky.app user tag   — targets another user
///   2. A "mapky" universal tag          — targets an external URI
///   3. An "eventky.app" universal tag   — targets the same external URI with
///      identical content, intentionally producing the **same `tag_id`** as
///      the mapky tag.  This is the adversarial case that exposed the bug:
///      without an app-scoped graph lookup, deleting the mapky tag could
///      accidentally operate on the eventky.app TAGGED relationship instead.
///
/// The test verifies that deleting each tag removes only that tag and leaves
/// the remaining two untouched.
#[tokio_shared_rt::test(shared)]
async fn test_universal_tag_del_scoped_to_app_namespace() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // ── Users ────────────────────────────────────────────────────────────────
    let tagger_kp = Keypair::random();
    let tagger_user = PubkyAppUser {
        bio: Some("test_universal_tag_del_scoped".to_string()),
        image: None,
        links: None,
        name: "Watcher:MultiAppDel:Tagger".to_string(),
        status: None,
    };
    test.create_user(&tagger_kp, &tagger_user).await?;

    let target_kp = Keypair::random();
    let target_user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Watcher:MultiAppDel:Target".to_string(),
        status: None,
    };
    let target_user_id = test.create_user(&target_kp, &target_user).await?;

    let label = "shared-label";

    // ── Tag 1: standard pubky.app user tag ──────────────────────────────────
    let pubky_app_tag = PubkyAppTag {
        uri: format!("pubky://{target_user_id}/pub/pubky.app/profile.json"),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let pubky_app_path = pubky_app_tag.hs_path();
    test.put(&tagger_kp, &pubky_app_path, &pubky_app_tag)
        .await?;

    // ── Tags 2 & 3: two universal tags with *identical* content ─────────────
    // Identical content → identical tag_id.  This is the adversarial case:
    // both TAGGED relationships will carry the same `id` property but
    // different `app` properties ("mapky" vs "eventky.app").
    let external_uri = "https://example.com/multi-app-del-target";
    let resource_id = compute_resource_id(external_uri);

    let shared_tag = PubkyAppTag {
        uri: external_uri.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let shared_tag_id = shared_tag.create_id();

    let mapky_path: ResourcePath = format!("/pub/mapky/tags/{shared_tag_id}").parse()?;
    let eventky_path: ResourcePath = format!("/pub/eventky.app/tags/{shared_tag_id}").parse()?;

    test.put(&tagger_kp, &mapky_path, &shared_tag).await?;
    test.put(&tagger_kp, &eventky_path, &shared_tag).await?;

    // ── Baseline assertions ──────────────────────────────────────────────────
    // Both universal tags should exist as separate TAGGED relationships.
    let tag_count = count_resource_tags(&resource_id).await?;
    assert_eq!(
        tag_count, 2,
        "Should have 2 TAGGED relationships (one per app)"
    );

    let mapky_tag = find_resource_tag_by_app(&resource_id, label, "mapky").await?;
    assert!(
        mapky_tag.is_some(),
        "mapky tag should exist before any deletions"
    );

    let eventky_tag = find_resource_tag_by_app(&resource_id, label, "eventky.app").await?;
    assert!(
        eventky_tag.is_some(),
        "eventky.app tag should exist before any deletions"
    );

    let user_tag_before = find_user_tag(&target_user_id, label).await?;
    assert!(
        user_tag_before.is_some(),
        "pubky.app user tag should exist before any deletions"
    );

    // ── Step 1: Delete the mapky tag ─────────────────────────────────────────
    test.del(&tagger_kp, &mapky_path).await?;

    let tag_count_after_mapky = count_resource_tags(&resource_id).await?;
    assert_eq!(
        tag_count_after_mapky, 1,
        "Should have 1 TAGGED relationship after mapky delete"
    );

    let mapky_tag_after = find_resource_tag_by_app(&resource_id, label, "mapky").await?;
    assert!(
        mapky_tag_after.is_none(),
        "mapky tag should be gone after delete"
    );

    let eventky_tag_after_mapky =
        find_resource_tag_by_app(&resource_id, label, "eventky.app").await?;
    assert!(
        eventky_tag_after_mapky.is_some(),
        "eventky.app tag must NOT be affected by mapky delete"
    );

    let user_tag_after_mapky = find_user_tag(&target_user_id, label).await?;
    assert!(
        user_tag_after_mapky.is_some(),
        "pubky.app user tag must NOT be affected by mapky delete"
    );

    // ── Step 2: Delete the eventky.app tag ──────────────────────────────────
    test.del(&tagger_kp, &eventky_path).await?;

    let tag_count_after_eventky = count_resource_tags(&resource_id).await?;
    assert_eq!(
        tag_count_after_eventky, 0,
        "Should have 0 TAGGED relationships after both universal deletes"
    );

    let eventky_tag_after = find_resource_tag_by_app(&resource_id, label, "eventky.app").await?;
    assert!(
        eventky_tag_after.is_none(),
        "eventky.app tag should be gone after delete"
    );

    // Resource-level Redis index should reflect zero taggers
    let cache_tags =
        TagResource::get_from_index(&resource_id, None, None, None, None, None, false).await?;
    let is_empty = cache_tags.is_none_or(|v| v.is_empty() || v[0].taggers_count == 0);
    assert!(
        is_empty,
        "TagResource cache should be empty after all universal tag deletions"
    );

    let user_tag_after_eventky = find_user_tag(&target_user_id, label).await?;
    assert!(
        user_tag_after_eventky.is_some(),
        "pubky.app user tag must NOT be affected by eventky.app delete"
    );

    // ── Step 3: Delete the standard pubky.app user tag ─────────────────────
    test.del(&tagger_kp, &pubky_app_path).await?;

    let user_tag_final = find_user_tag(&target_user_id, label).await?;
    assert!(
        user_tag_final.is_none(),
        "pubky.app user tag should be gone after its own delete"
    );

    // ── Cleanup ──────────────────────────────────────────────────────────────
    test.cleanup_user(&tagger_kp).await?;
    test.cleanup_user(&target_kp).await?;

    Ok(())
}
