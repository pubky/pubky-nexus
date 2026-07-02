use super::resource_utils::{
    check_resource_in_sorted_set, compute_resource_id, count_resource_tags,
};
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use nexus_common::db::kv::ScoreAction;
use nexus_common::models::resource::stream::ResourceStream;
use nexus_common::models::resource::tag::TagResource;
use nexus_common::models::tag::traits::{TagCollection, TaggersCollection};
use nexus_watcher::events::handlers;
use pubky::Keypair;
use pubky::ResourcePath;
use pubky_app_specs::traits::HashId;
use pubky_app_specs::{PubkyAppTag, PubkyAppUser};

/// Simulate a retry of a resource tag del after a partial failure where the
/// Redis cleanup succeeded but the graph deletion failed. On retry, the
/// taggers counts must NOT be decremented again (guarded by the tagger set
/// membership check), so a still-tagged resource must keep count 1 and stay
/// in the resource timelines.
#[tokio_shared_rt::test(shared)]
async fn test_resource_tag_del_retry_no_double_decrement() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let target_uri = "https://example.com/del-retry-test";
    let label = "retry-res-label";
    let app = "mapky";
    let resource_id = compute_resource_id(target_uri);

    // Two users tag the same external URI with the same label from the same app
    let user1_kp = Keypair::random();
    let user1 = PubkyAppUser {
        bio: Some("resource_del_retry_user_1".to_string()),
        image: None,
        links: None,
        name: "Watcher:ResourceDelRetry:User1".to_string(),
        status: None,
    };
    let user1_id = test.create_user(&user1_kp, &user1).await?;

    let tag1 = PubkyAppTag {
        uri: target_uri.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag1_id = tag1.create_id();
    let path1: ResourcePath = format!("/pub/{app}/tags/{tag1_id}").parse()?;
    test.put(&user1_kp, &path1, &tag1).await?;

    let user2_kp = Keypair::random();
    let user2 = PubkyAppUser {
        bio: Some("resource_del_retry_user_2".to_string()),
        image: None,
        links: None,
        name: "Watcher:ResourceDelRetry:User2".to_string(),
        status: None,
    };
    let _user2_id = test.create_user(&user2_kp, &user2).await?;

    let tag2 = PubkyAppTag {
        uri: target_uri.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag2_id = tag2.create_id();
    let path2: ResourcePath = format!("/pub/{app}/tags/{tag2_id}").parse()?;
    test.put(&user2_kp, &path2, &tag2).await?;

    // Verify initial state: 2 TAGGED edges, taggers counts at 2
    assert_eq!(count_resource_tags(&resource_id).await?, 2);
    let global_count =
        check_resource_in_sorted_set(&["Resources", "Global", "TaggersCount"], &resource_id)
            .await?;
    assert_eq!(global_count, Some(2));

    // Simulate partial completion of a previous del attempt for user1's tag:
    // the Redis cleanup (SREM + all decrements) completed, but the graph
    // deletion failed, so the TAGGED edge is still present
    TagResource(vec![user1_id.clone()])
        .del_from_index(&resource_id, None, label)
        .await?;
    TagResource::update_index_score(&resource_id, None, label, ScoreAction::Decrement(1.0)).await?;
    ResourceStream::update_global_taggers_count(&resource_id, ScoreAction::Decrement(1.0)).await?;
    ResourceStream::update_tag_taggers_count(label, &resource_id, ScoreAction::Decrement(1.0))
        .await?;
    ResourceStream::update_app_taggers_count(app, &resource_id, ScoreAction::Decrement(1.0))
        .await?;
    ResourceStream::update_app_tag_taggers_count(
        app,
        label,
        &resource_id,
        ScoreAction::Decrement(1.0),
    )
    .await?;

    // Verify simulated state: graph still has both edges, counts already at 1
    assert_eq!(count_resource_tags(&resource_id).await?, 2);
    let global_count =
        check_resource_in_sorted_set(&["Resources", "Global", "TaggersCount"], &resource_id)
            .await?;
    assert_eq!(global_count, Some(1));

    // Retry: re-run the same delete event by calling the del handler directly.
    // It must delete the graph edge without decrementing the counts again
    let tag_uri = format!("pubky://{user1_id}/pub/{app}/tags/{tag1_id}");
    handlers::tag::del(&tag_uri).await?;

    // Only user2's TAGGED edge should remain in the graph
    assert_eq!(count_resource_tags(&resource_id).await?, 1);

    // Taggers count must be 1 (not 0): the retry must not double-decrement
    let cache_tags = <TagResource as TagCollection>::get_from_index(
        &resource_id,
        None,
        None,
        None,
        None,
        None,
        false,
    )
    .await?;
    let details = cache_tags.expect("TagResource cache should still exist");
    assert_eq!(details.len(), 1, "Should still have 1 label");
    assert_eq!(details[0].label, label);
    assert_eq!(
        details[0].taggers_count, 1,
        "Taggers count must be 1 after retry, not double-decremented to 0"
    );

    // All taggers counts must be 1 (not 0)
    for count_key_parts in [
        vec!["Resources", "Global", "TaggersCount"],
        vec!["Resources", "Tag", label, "TaggersCount"],
        vec!["Resources", "App", app, "TaggersCount"],
        vec!["Resources", "App", app, "Tag", label, "TaggersCount"],
    ] {
        let count = check_resource_in_sorted_set(&count_key_parts, &resource_id).await?;
        assert_eq!(
            count,
            Some(1),
            "Taggers count {count_key_parts:?} must be 1 after retry"
        );
    }

    // The still-tagged resource must NOT be evicted from the timelines
    for timeline_key_parts in [
        vec!["Resources", "Global", "Timeline"],
        vec!["Resources", "Tag", label, "Timeline"],
        vec!["Resources", "App", app, "Timeline"],
        vec!["Resources", "App", app, "Tag", label, "Timeline"],
    ] {
        let member = check_resource_in_sorted_set(&timeline_key_parts, &resource_id).await?;
        assert!(
            member.is_some(),
            "Resource must remain in timeline {timeline_key_parts:?} after retry"
        );
    }

    // Cleanup: user1's homeserver file still exists (graph edge already gone,
    // the DEL event is an idempotent no-op), then really delete user2's tag
    test.del(&user1_kp, &path1).await?;
    test.del(&user2_kp, &path2).await?;
    test.cleanup_user(&user1_kp).await?;
    test.cleanup_user(&user2_kp).await?;

    Ok(())
}
