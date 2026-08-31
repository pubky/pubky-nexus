use super::utils::{long_post, short_post};
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::{
    models::notification::{Notification, NotificationBody},
    types::Pagination,
};
use pubky::Keypair;
use pubky_app_specs::{post_uri_builder, PubkyAppPostKind, PubkyAppTag, PubkyAppUser};

/// Regression: a kind-only edit must refresh the cached kind used by notifications.
#[tokio_shared_rt::test(shared)]
async fn test_kind_only_edit_refreshes_notification_post_kind() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Watcher:KindEditRefresh:Author".to_string(),
        status: None,
    };
    let author_id = test.create_user(&author_kp, &author).await?;

    let tagger_kp = Keypair::random();
    let tagger = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Watcher:KindEditRefresh:Tagger".to_string(),
        status: None,
    };
    test.create_user(&tagger_kp, &tagger).await?;

    // Edit Short -> Long with identical content, so only the kind changes.
    let content = "identical content across the kind change";
    let (post_id, post_path) = test.create_post(&author_kp, &short_post(content)).await?;
    test.put(&author_kp, &post_path, &long_post(content))
        .await?;

    // Tagger tags the (now Long) post; the author is notified.
    let tag = PubkyAppTag {
        uri: post_uri_builder(author_id.clone(), post_id.clone()),
        label: "longform".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    test.put(&tagger_kp, &tag.hs_path(), tag).await?;

    let notifications = Notification::get_by_id(&author_id, Pagination::default())
        .await
        .unwrap();
    assert_eq!(
        notifications.len(),
        1,
        "Author should have the tag notification"
    );

    if let NotificationBody::TagPost { post_kind, .. } = &notifications[0].body {
        assert_eq!(
            post_kind,
            &PubkyAppPostKind::Long,
            "kind-only edit must refresh the cache so the notification reports Long, not the stale Short"
        );
    } else {
        panic!("Expected a TagPost notification, found something else");
    }

    Ok(())
}
