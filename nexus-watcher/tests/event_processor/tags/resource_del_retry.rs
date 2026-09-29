use super::resource_utils::{
    app_stream_resource_ids, compute_resource_id, count_resource_tags, resource_exists_in_graph,
    resource_label_score,
};
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use nexus_common::db::kv::ScoreAction;
use nexus_common::models::resource::tag::TagResource;
use nexus_common::models::tag::traits::{TagCollection, TaggersCollection};
use nexus_watcher::events::handlers;
use pubky::Keypair;
use pubky::ResourcePath;
use pubky_app_specs::traits::HashId;
use pubky_app_specs::{PubkyAppTag, PubkyAppUser};

/// Simulate a retry of a resource tag del after a partial failure where the
/// Redis cleanup succeeded but the graph deletion failed. On retry, the label
/// score must NOT be decremented again (guarded by the app-scoped tagger set
/// membership check), so a still-tagged resource must keep count 1. The
/// stream is served from the graph, so the resource stays listed for as long
/// as a TAGGED edge remains, whatever Redis says.
#[tokio_shared_rt::test(shared)]
async fn test_resource_tag_del_retry_no_double_decrement() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let target_uri = "https://example.com/del-retry-test";
    let label = "retry-res-label";
    let app = &format!("delretry{}", Utc::now().timestamp_millis());
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

    // Verify initial state: 2 TAGGED edges, label score at 2
    assert_eq!(count_resource_tags(&resource_id).await?, 2);
    assert_eq!(resource_label_score(&resource_id, label).await?, Some(2));
    assert_eq!(
        app_stream_resource_ids(app).await?,
        vec![resource_id.clone()]
    );

    // Simulate partial completion of a previous del attempt for user1's tag:
    // the Redis cleanup (SREMs + label score decrement) completed, but the
    // graph deletion failed, so the TAGGED edge is still present
    TagResource(vec![user1_id.clone()])
        .del_from_index(&resource_id, None, label)
        .await?;
    TagResource(vec![user1_id.clone()])
        .del_from_index(&resource_id, Some(app), label)
        .await?;
    TagResource::update_index_score(&resource_id, None, label, ScoreAction::Decrement(1.0)).await?;

    // Verify simulated state: graph still has both edges, score already at 1
    assert_eq!(count_resource_tags(&resource_id).await?, 2);
    assert_eq!(resource_label_score(&resource_id, label).await?, Some(1));

    // Retry: re-run the same delete event by calling the del handler directly.
    // It must delete the graph edge without decrementing the score again
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

    // The still-tagged resource stays in the graph-served stream
    assert_eq!(
        app_stream_resource_ids(app).await?,
        vec![resource_id.clone()]
    );

    // Cleanup: user1's homeserver file still exists (graph edge already gone,
    // the DEL event is an idempotent no-op), then really delete user2's tag
    test.del(&user1_kp, &path1).await?;
    test.del(&user2_kp, &path2).await?;
    test.cleanup_user(&user1_kp).await?;
    test.cleanup_user(&user2_kp).await?;

    Ok(())
}

/// The same user tags the same external URI with the same label from TWO
/// different app namespaces, creating two app-scoped TAGGED edges whose put
/// events each incremented the label score. The retry gate must be
/// app-scoped: a retry of the first app's delete must not double-decrement,
/// and the second app's delete must still run its decrement so the score
/// reaches zero. The stream follows the graph: the resource is listed under
/// each app while its edge exists and vanishes with the last edge.
#[tokio_shared_rt::test(shared)]
async fn test_resource_tag_del_multi_app_full_cleanup() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let target_uri = "https://example.com/multi-app-del-test";
    let label = "multi-app-res-label";
    let suffix = Utc::now().timestamp_millis();
    let app1 = &format!("multiappa{suffix}");
    let app2 = &format!("multiappb{suffix}");
    let resource_id = compute_resource_id(target_uri);

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("resource_del_multi_app_user".to_string()),
        image: None,
        links: None,
        name: "Watcher:ResourceDelMultiApp:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    // Tag the URI from app1
    let tag1 = PubkyAppTag {
        uri: target_uri.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag1_id = tag1.create_id();
    let path1: ResourcePath = format!("/pub/{app1}/tags/{tag1_id}").parse()?;
    test.put(&user_kp, &path1, &tag1).await?;

    // Tag the same URI with the same label from app2
    let tag2 = PubkyAppTag {
        uri: target_uri.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis() + 1,
    };
    let tag2_id = tag2.create_id();
    let path2: ResourcePath = format!("/pub/{app2}/tags/{tag2_id}").parse()?;
    test.put(&user_kp, &path2, &tag2).await?;

    // Two app-scoped TAGGED edges; the per-edge label score ran twice
    assert_eq!(count_resource_tags(&resource_id).await?, 2);
    assert_eq!(resource_label_score(&resource_id, label).await?, Some(2));
    for app in [app1, app2] {
        assert_eq!(
            app_stream_resource_ids(app).await?,
            vec![resource_id.clone()],
            "resource must be listed under {app}"
        );
    }

    // Simulate partial completion of a del attempt for the app1 tag: the
    // Redis cleanup (SREMs + label score decrement) completed, but the graph
    // deletion failed, so the app1 TAGGED edge is still present
    TagResource(vec![user_id.clone()])
        .del_from_index(&resource_id, None, label)
        .await?;
    TagResource(vec![user_id.clone()])
        .del_from_index(&resource_id, Some(app1), label)
        .await?;
    TagResource::update_index_score(&resource_id, None, label, ScoreAction::Decrement(1.0)).await?;

    // Retry the app1 delete: it must not decrement anything again, only
    // finish the pending graph deletion
    let tag1_uri = format!("pubky://{user_id}/pub/{app1}/tags/{tag1_id}");
    handlers::tag::del(&tag1_uri).await?;

    // Only the app2 TAGGED edge remains, and the retry did not
    // double-decrement the label score
    assert_eq!(count_resource_tags(&resource_id).await?, 1);
    assert_eq!(
        resource_label_score(&resource_id, label).await?,
        Some(1),
        "label score must be 1 after the app1 retry"
    );
    assert_eq!(
        app_stream_resource_ids(app1).await?,
        Vec::<String>::new(),
        "the app1 stream follows the deleted edge"
    );
    assert_eq!(
        app_stream_resource_ids(app2).await?,
        vec![resource_id.clone()],
        "the app2 stream still lists the resource"
    );

    // Delete the app2 tag: its app-scoped tagger set still holds the member,
    // so the decrement must run and zero out the score
    test.del(&user_kp, &path2).await?;

    assert_eq!(count_resource_tags(&resource_id).await?, 0);
    assert!(
        !resource_exists_in_graph(&resource_id).await?,
        "orphaned Resource node must be removed with its last tag"
    );
    assert_eq!(
        resource_label_score(&resource_id, label)
            .await?
            .unwrap_or(0),
        0,
        "label score must be 0 after both deletes"
    );
    for app in [app1, app2] {
        assert_eq!(
            app_stream_resource_ids(app).await?,
            Vec::<String>::new(),
            "resource must be gone from the {app} stream"
        );
    }

    // Cleanup: the app1 homeserver file still exists (its graph edge is
    // already gone, so the DEL event is an idempotent no-op)
    test.del(&user_kp, &path1).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}
