use super::resource_utils::{compute_resource_id, find_resource_tag, resource_exists_in_graph};
use crate::event_processor::posts::utils::test_user;
use crate::event_processor::users::utils::find_user_counts;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::event::EventLine;
use nexus_watcher::events::retry::event::RetryEvent;
use nexus_watcher::events::retry::IndexKey;
use pubky::Keypair;
use pubky::ResourcePath;
use pubky_social_specs::legacy_v0::traits::HashId;
use pubky_social_specs::legacy_v0::PubkyAppTag;

/// The `social` namespace is reserved for the spec: a v0 tag written at an epoch-less
/// `/pub/social/tags/{id}` is not a universal tag of an app named `social`. The watcher skips
/// it at parse time, so nothing is fetched, indexed, queued or logged.
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_epochless_social_tag_is_not_indexed() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let tagger = test_user("Watcher:ReservedSocial:Tagger", "test_epochless_social_tag");
    let tagger_id = test.create_user(&user_kp, &tagger).await?;
    let counts_before = find_user_counts(&tagger_id).await;
    let (_, log_cursor_before) = EventLine::get_from_index(None, usize::MAX).await?;

    let target_uri = "https://example.com/reserved-social-namespace";
    let label = "reserved";
    let tag = PubkyAppTag {
        uri: target_uri.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_id = tag.create_id();
    let resource_id = compute_resource_id(target_uri);

    let tag_path: ResourcePath = format!("/pub/social/tags/{tag_id}").parse()?;
    test.put(&user_kp, &tag_path, &tag).await?;

    // Graph: no resource tag and no Resource node
    assert!(
        find_resource_tag(&resource_id, label).await?.is_none(),
        "an epoch-less social tag must not become a resource tag"
    );
    assert!(
        !resource_exists_in_graph(&resource_id).await?,
        "an epoch-less social tag must not create a Resource node"
    );

    // The tagger's counts unchanged
    let counts_after = find_user_counts(&tagger_id).await;
    assert_eq!(counts_after.tagged, counts_before.tagged);
    assert_eq!(counts_after.tags, counts_before.tags);

    // Watcher state: no retry entry and no stored event line
    let tag_uri = format!("pubky://{tagger_id}{tag_path}");
    assert!(
        !RetryEvent::check_index_key(&IndexKey::for_uri(&tag_uri)).await?,
        "a skipped event must not reach the retry queue"
    );
    let (new_lines, _) = EventLine::get_from_index(Some(log_cursor_before), usize::MAX).await?;
    assert!(
        !new_lines.iter().any(|line| line.ends_with(&tag_uri)),
        "a skipped event must not be stored in the event log"
    );

    test.del(&user_kp, &tag_path).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}
