use super::resource_utils::{compute_resource_id, count_resource_tags, resource_exists_in_graph};
use crate::event_processor::utils::watcher::{
    retrieve_and_handle_event_line, HomeserverHashIdPath, WatcherTest,
};
use anyhow::Result;
use chrono::Utc;
use nexus_common::db::exec_single_row;
use nexus_common::db::graph::Query;
use nexus_common::models::event::resource_changes::ResourceChange;
use nexus_common::universal_tag::normalize::normalize_uri;
use nexus_watcher::events::retry::RetryScheduler;
use nexus_watcher::EventProcessorError;
use pubky::Keypair;
use pubky_social_specs::legacy_v0::{
    follow_uri_builder, mute_uri_builder, tag_uri_builder, traits::HashId, ParsedUri, PubkyAppTag,
    PubkyAppUser, Resource,
};

/// A pubky.app tag on a v0 kind with no id (`pubky.app/posts/`), which the v0 parser reads as
/// `Unknown`, is rejected deterministically and never retried. An unknown kind there becomes a
/// `Resource` tag instead (`resource_put.rs`).
#[tokio_shared_rt::test(shared)]
async fn test_tag_unknown_target_is_not_retried() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_tag_unknown_target_is_not_retried".to_string()),
        image: None,
        links: None,
        name: "Watcher:Tag:UnknownTarget".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    // Switch OFF event processing so the tag is handled only by the call below
    test = test.remove_event_processing().await;

    let tag = PubkyAppTag {
        uri: format!("pubky://{user_id}/pub/pubky.app/posts/"),
        label: "unknown_target".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    assert_eq!(
        ParsedUri::try_from(tag.uri.as_str())
            .expect("tag target should parse")
            .resource,
        Resource::Unknown
    );
    let tag_id = tag.create_id();
    test.put(&keypair, &tag.hs_path(), &tag).await?;

    let tag_event = format!("PUT pubky://{user_id}/pub/pubky.app/tags/{tag_id}");
    let event_handler = test.event_processor_runner.event_handler.clone();
    let err = retrieve_and_handle_event_line(&tag_event, &test.homeserver_id, event_handler)
        .await
        .expect_err("tag with an unknown target must be rejected");

    assert!(
        !RetryScheduler::should_enqueue_related_event(&err),
        "deterministic rejection must not be retried, got {err:?}"
    );
    assert!(matches!(err, EventProcessorError::UnsupportedTarget(_)));
    assert!(
        !resource_exists_in_graph(&compute_resource_id(&tag.uri)).await?,
        "a rejected tag must not create a Resource node"
    );

    Ok(())
}

/// A tag on a follow, a mute or another tag names something inside Nexus's own namespace that it
/// does not tag, so it is rejected deterministically: no tag, no `Resource`, and the event is not
/// queued for retry.
#[tokio_shared_rt::test(shared)]
async fn test_tag_on_follow_mute_or_tag_is_not_retried() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_tag_on_follow_mute_or_tag_is_not_retried".to_string()),
        image: None,
        links: None,
        name: "Watcher:Tag:SocialKindTarget".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;
    let other_id = Keypair::random().public_key().to_z32();

    // Switch OFF event processing so each tag is handled only by the call below
    test = test.remove_event_processing().await;

    let targets = [
        follow_uri_builder(user_id.clone(), other_id.clone()),
        mute_uri_builder(user_id.clone(), other_id),
        tag_uri_builder(user_id.clone(), "8Z8CWH8NVYQY39ZEBFGKQWWEKG".to_string()),
    ];
    for target_uri in &targets {
        let tag = PubkyAppTag {
            uri: target_uri.clone(),
            label: "social_kind_target".to_string(),
            created_at: Utc::now().timestamp_millis(),
        };
        let tag_path = tag.hs_path();
        test.put(&keypair, &tag_path, &tag).await?;

        let tag_event = format!("PUT pubky://{user_id}{tag_path}");
        let event_handler = test.event_processor_runner.event_handler.clone();
        let err = retrieve_and_handle_event_line(&tag_event, &test.homeserver_id, event_handler)
            .await
            .expect_err("a tag on a social kind Nexus doesn't tag must be rejected");

        assert!(
            matches!(err, EventProcessorError::UnsupportedTarget(_)),
            "{target_uri}: expected UnsupportedTarget, got {err:?}"
        );
        assert!(
            !RetryScheduler::should_enqueue_related_event(&err),
            "{target_uri}: deterministic rejection must not be retried, got {err:?}"
        );
        assert!(
            !resource_exists_in_graph(&compute_resource_id(target_uri)).await?,
            "{target_uri}: a rejected tag must not create a Resource node"
        );
    }

    Ok(())
}

/// A resource tag DEL whose Resource has no `uri` (corrupt or imported data) has no feed key to
/// publish. A retry would read the same node, so it is a data anomaly, never retried, as the
/// seed skips the same edge: no feed line, the edge stays, and the batch goes on.
#[tokio_shared_rt::test(shared)]
async fn test_tag_del_of_a_resource_without_uri_is_not_retried() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_tag_del_of_a_resource_without_uri_is_not_retried".to_string()),
        image: None,
        links: None,
        name: "Watcher:Tag:ResourceWithoutUri".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    let target = format!("https://example.com/resource-without-uri/{user_id}");
    let tag = PubkyAppTag {
        uri: target.clone(),
        label: "no_uri".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();
    test.put(&keypair, &tag_path, &tag).await?;
    let resource_id = compute_resource_id(&target);
    assert_eq!(count_resource_tags(&resource_id).await?, 1);

    let set_uri = |uri: Option<String>| {
        Query::new(
            "test_resource_without_uri",
            "MATCH (r:Resource {id: $resource_id}) SET r.uri = $uri",
        )
        .param("resource_id", resource_id.as_str())
        .param("uri", uri)
    };
    exec_single_row(set_uri(None)).await?;

    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;
    let tag_event = format!("DEL pubky://{user_id}{tag_path}");
    let event_handler = test.event_processor_runner.event_handler.clone();
    let err =
        retrieve_and_handle_event_line(&tag_event, &test.homeserver_id, event_handler.clone())
            .await
            .expect_err("a tag DEL of a Resource with no uri must fail");

    assert!(
        matches!(err, EventProcessorError::DataAnomaly(_)),
        "expected DataAnomaly, got {err:?}"
    );
    assert!(
        !RetryScheduler::should_enqueue_related_event(&err),
        "a Resource with no uri must not be retried, got {err:?}"
    );
    assert!(!err.should_not_retry_now(), "the batch goes on: {err:?}");
    let (page, _, _) = ResourceChange::get_from_index(start, 1000).await?;
    assert!(
        page.lines.iter().all(|line| line.author != user_id),
        "no feed line without a uri: {:?}",
        page.lines
    );
    assert_eq!(
        count_resource_tags(&resource_id).await?,
        1,
        "the edge is left as it is"
    );

    // Cleanup: give the Resource its uri back so the DEL removes the edge and the Resource
    let (normalized, _) = normalize_uri(&target).expect("the target normalizes");
    exec_single_row(set_uri(Some(normalized))).await?;
    retrieve_and_handle_event_line(&tag_event, &test.homeserver_id, event_handler).await?;
    assert!(!resource_exists_in_graph(&resource_id).await?);
    test.cleanup_user(&keypair).await?;

    Ok(())
}
