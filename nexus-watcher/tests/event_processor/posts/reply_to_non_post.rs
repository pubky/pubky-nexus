use super::utils::find_post_details;
use crate::event_processor::users::utils::find_user_counts;
use crate::event_processor::utils::watcher::{retrieve_and_handle_event_line, WatcherTest};
use anyhow::Result;
use nexus_common::models::post::PostParents;
use nexus_watcher::events::retry::RetryScheduler;
use nexus_watcher::EventProcessorError;
use pubky::Keypair;
use pubky_social_specs::legacy_v0::{
    file_uri_builder, follow_uri_builder, post_uri_builder, tag_uri_builder, user_uri_builder,
    PubkyAppPost, PubkyAppPostKind, PubkyAppUser,
};

/// A reply whose parent is a file, a profile or a `pubky.app` tag is rejected deterministically:
/// no reply edge can ever point at one, so the event must not be queued for retry, and the reply
/// is neither indexed nor counted.
#[tokio_shared_rt::test(shared)]
async fn test_reply_to_a_non_post_is_rejected_and_not_retried() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_reply_to_a_non_post_is_rejected_and_not_retried".to_string()),
        image: None,
        links: None,
        name: "Watcher:ReplyToNonPost:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    // Switch OFF event processing so each reply is handled only by the call below
    test = test.remove_event_processing().await;

    let parents = [
        (
            "file",
            file_uri_builder(user_id.clone(), "0033SSE3B1FQ0".into()),
        ),
        ("profile", user_uri_builder(user_id.clone())),
        (
            "tag",
            tag_uri_builder(user_id.clone(), "8Z8CWH8NVYQY39ZEBFGKQWWEKG".into()),
        ),
    ];
    for (kind, parent) in parents {
        let reply = PubkyAppPost {
            content: format!("Watcher:ReplyToNonPost:User:Reply:{kind}"),
            kind: PubkyAppPostKind::Short,
            parent: Some(parent),
            embed: None,
            attachments: None,
            lock: None,
        };
        let (reply_id, _reply_path) = test.create_post(&user_kp, &reply).await?;

        let reply_event = format!(
            "PUT {}",
            post_uri_builder(user_id.clone(), reply_id.clone())
        );
        let event_handler = test.event_processor_runner.event_handler.clone();
        let err = retrieve_and_handle_event_line(&reply_event, event_handler)
            .await
            .expect_err("a reply to a non-post must be rejected");

        assert!(
            !RetryScheduler::should_enqueue_related_event(&err),
            "deterministic rejection must not be retried, got {err:?} ({kind})"
        );
        assert!(
            matches!(err, EventProcessorError::UnsupportedTarget(_)),
            "unexpected error for a {kind} parent: {err}"
        );

        // GRAPH_OP: the reply never reaches the graph
        assert!(
            find_post_details(&user_id, &reply_id).await.is_err(),
            "a reply to a {kind} must not be indexed"
        );
    }

    // CACHE_OP: nothing was counted
    let user_counts = find_user_counts(&user_id).await;
    assert_eq!(user_counts.posts, 0);
    assert_eq!(user_counts.replies, 0);

    Ok(())
}

/// A reply whose parent is another app's object, or a `pubky.app` resource Nexus never
/// references, is indexed with no reply edge.
#[tokio_shared_rt::test(shared)]
async fn test_reply_to_another_apps_object_is_indexed_without_a_reply_edge() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_reply_to_another_apps_object_is_indexed_without_a_reply_edge".to_string()),
        image: None,
        links: None,
        name: "Watcher:ReplyToNonPost:Other".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    // Switch OFF event processing so each reply is handled only by the call below
    test = test.remove_event_processing().await;

    let parents = [
        (
            "another app's tag",
            format!("pubky://{user_id}/pub/mapky/tags/8Z8CWH8NVYQY39ZEBFGKQWWEKG"),
        ),
        (
            "follow",
            follow_uri_builder(user_id.clone(), user_id.clone()),
        ),
    ];
    for (kind, parent) in parents {
        let reply = PubkyAppPost {
            content: format!("Watcher:ReplyToNonPost:Other:Reply:{kind}"),
            kind: PubkyAppPostKind::Short,
            parent: Some(parent),
            embed: None,
            attachments: None,
            lock: None,
        };
        let (reply_id, _reply_path) = test.create_post(&user_kp, &reply).await?;

        let reply_event = format!(
            "PUT {}",
            post_uri_builder(user_id.clone(), reply_id.clone())
        );
        let event_handler = test.event_processor_runner.event_handler.clone();
        retrieve_and_handle_event_line(&reply_event, event_handler)
            .await
            .unwrap_or_else(|e| panic!("a reply to a {kind} must be indexed: {e}"));

        // GRAPH_OP: indexed, with no reply edge
        assert!(
            find_post_details(&user_id, &reply_id).await.is_ok(),
            "a reply to a {kind} must be indexed"
        );
        let parents = PostParents::get_from_graph(&user_id, &reply_id)
            .await?
            .expect("the reply is in the graph");
        assert_eq!(parents.replied, None, "{kind}");
    }

    Ok(())
}
