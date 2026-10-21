use crate::event_processor::utils::watcher::{
    retrieve_and_handle_event_line, HomeserverHashIdPath, WatcherTest,
};
use anyhow::Result;
use chrono::Utc;
use nexus_watcher::events::retry::RetryScheduler;
use nexus_watcher::EventProcessorError;
use pubky::Keypair;
use pubky_social_specs::legacy_v0::{
    traits::HashId, ParsedUri, PubkyAppTag, PubkyAppUser, Resource,
};

/// A pubky.app tag whose target parses to neither a Post nor a User is rejected
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
        err.to_string().contains("not Post or User"),
        "unexpected error: {err}"
    );
    assert!(
        !RetryScheduler::should_enqueue_related_event(&err),
        "deterministic rejection must not be retried, got {err:?}"
    );
    assert!(matches!(err, EventProcessorError::UnsupportedTarget(_)));

    Ok(())
}

/// A pubky.app tag on a URL is rejected deterministically: tags on URLs are a feature of other
/// apps' folders only, so the event must not be queued for retry.
#[tokio_shared_rt::test(shared)]
async fn test_tag_url_target_in_pubky_app_is_not_retried() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_tag_url_target_in_pubky_app_is_not_retried".to_string()),
        image: None,
        links: None,
        name: "Watcher:Tag:UrlTarget".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    // Switch OFF event processing so the tag is handled only by the call below
    test = test.remove_event_processing().await;

    let tag = PubkyAppTag {
        uri: "https://example.com/article".to_string(),
        label: "url_target".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_id = tag.create_id();
    test.put(&keypair, &tag.hs_path(), &tag).await?;

    let tag_event = format!("PUT pubky://{user_id}/pub/pubky.app/tags/{tag_id}");
    let event_handler = test.event_processor_runner.event_handler.clone();
    let err = retrieve_and_handle_event_line(&tag_event, event_handler)
        .await
        .expect_err("a pubky.app tag on a URL must be rejected");

    assert!(
        err.to_string().contains("not Post or User"),
        "unexpected error: {err}"
    );
    assert!(
        !RetryScheduler::should_enqueue_related_event(&err),
        "deterministic rejection must not be retried, got {err:?}"
    );
    assert!(matches!(err, EventProcessorError::UnsupportedTarget(_)));

    Ok(())
}
