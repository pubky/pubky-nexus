use super::resource_utils::{compute_resource_id, resource_exists_in_graph};
use crate::event_processor::utils::watcher::{
    retrieve_and_handle_event_line, HomeserverHashIdPath, WatcherTest,
};
use anyhow::Result;
use chrono::Utc;
use nexus_watcher::events::retry::RetryScheduler;
use nexus_watcher::EventProcessorError;
use pubky::Keypair;
use pubky_social_specs::legacy_v0::{
    follow_uri_builder, mute_uri_builder, tag_uri_builder, traits::HashId, ParsedUri, PubkyAppTag,
    PubkyAppUser, Resource,
};

/// A pubky.app tag whose target is an unknown `pubky.app` resource is rejected
/// deterministically, so it must not be queued for retry.
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
        uri: format!("pubky://{user_id}/pub/pubky.app/unknown_kind/0033SSE3B1FQ0"),
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
    let err = retrieve_and_handle_event_line(&tag_event, event_handler)
        .await
        .expect_err("tag with an unknown target must be rejected");

    assert!(
        !RetryScheduler::should_enqueue_related_event(&err),
        "deterministic rejection must not be retried, got {err:?}"
    );
    assert!(matches!(err, EventProcessorError::UnsupportedTarget(_)));

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
        let err = retrieve_and_handle_event_line(&tag_event, event_handler)
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
