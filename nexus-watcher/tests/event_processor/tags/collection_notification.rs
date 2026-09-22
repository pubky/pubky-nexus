use crate::event_processor::posts::utils::{collection_post, long_post};
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::{
    models::notification::{Notification, NotificationBody},
    types::Pagination,
};
use pubky::Keypair;
use pubky_app_specs::{post_uri_builder, PubkyAppPostKind, PubkyAppTag, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
async fn test_tag_untag_collection_notification() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    // Create first user (collection author)
    let author_kp = Keypair::random();
    let author_user = PubkyAppUser {
        bio: Some("test_tag_untag_collection_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:TagCollectionNotification:Author".to_string(),
        status: None,
    };
    let author_id = test.create_user(&author_kp, &author_user).await?;

    // Create second user (tagger)
    let tagger_kp = Keypair::random();
    let tagger_user = PubkyAppUser {
        bio: Some("test_tag_untag_collection_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:TagCollectionNotification:Tagger".to_string(),
        status: None,
    };
    let tagger_id = test.create_user(&tagger_kp, &tagger_user).await?;

    // Author creates a collection
    let post = collection_post("taggable");
    let (post_id, post_path) = test.create_post(&author_kp, &post).await?;
    let collection_uri = post_uri_builder(author_id.clone(), post_id.clone());

    // Tagger adds a tag to the collection
    let label = "curated";
    let tag = PubkyAppTag {
        uri: collection_uri.clone(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();
    test.put(&tagger_kp, &tag_path, tag).await?;

    // Verify the TagCollection notification was created
    let notifications = Notification::get_by_id(&author_id, Pagination::default())
        .await
        .unwrap();
    assert_eq!(
        notifications.len(),
        1,
        "Collection author should have 1 notification for the new tag"
    );

    if let NotificationBody::TagPost {
        tagged_by,
        tag_label,
        post_uri,
        post_kind,
    } = &notifications[0].body
    {
        assert_eq!(
            post_kind,
            &PubkyAppPostKind::Collection,
            "Tagging a collection should tag the notification with post_kind = Collection"
        );
        assert_eq!(
            tagged_by, &tagger_id,
            "Notification should contain the correct tagger"
        );
        assert_eq!(
            tag_label, label,
            "Notification should contain the correct tag label"
        );
        assert_eq!(
            post_uri, &collection_uri,
            "Notification should contain the correct collection URI"
        );
    } else {
        panic!("Expected a TagPost notification, found something else");
    }

    // Delete the tag
    test.del(&tagger_kp, &tag_path).await?;

    // Check the author now also has an UntagCollection notification
    let notifications = Notification::get_by_id(&author_id, Pagination::default())
        .await
        .unwrap();
    assert_eq!(
        notifications.len(),
        2,
        "Collection author should have 2 notifications: tag + untag"
    );

    // The most recent notification (first in descending order) should be UntagPost
    if let NotificationBody::UntagPost {
        untagged_by,
        tag_label,
        post_uri,
        post_kind,
    } = &notifications[0].body
    {
        assert_eq!(
            post_kind,
            &PubkyAppPostKind::Collection,
            "Untagging a collection should tag the notification with post_kind = Collection"
        );
        assert_eq!(
            untagged_by, &tagger_id,
            "Notification should contain the correct untagger"
        );
        assert_eq!(
            tag_label, label,
            "Notification should contain the correct tag label"
        );
        assert_eq!(
            post_uri, &collection_uri,
            "Notification should contain the correct collection URI"
        );
    } else {
        panic!("Expected an UntagPost notification, found something else");
    }

    // Cleanup
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&author_kp).await?;
    test.cleanup_user(&tagger_kp).await?;

    Ok(())
}

/// Proves post_kind is not collection-only: tagging a Long article reports Long.
#[tokio_shared_rt::test(shared)]
async fn test_tag_article_notification_reports_long() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Watcher:TagArticleNotification:Author".to_string(),
        status: None,
    };
    let author_id = test.create_user(&author_kp, &author_user).await?;

    let tagger_kp = Keypair::random();
    let tagger_user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Watcher:TagArticleNotification:Tagger".to_string(),
        status: None,
    };
    test.create_user(&tagger_kp, &tagger_user).await?;

    // Author writes a Long article
    let (post_id, _post_path) = test
        .create_post(&author_kp, &long_post("An article"))
        .await?;
    let article_uri = post_uri_builder(author_id.clone(), post_id.clone());

    // Tagger tags the article
    let tag = PubkyAppTag {
        uri: article_uri.clone(),
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
        "Author should have 1 tag notification"
    );

    if let NotificationBody::TagPost { post_kind, .. } = &notifications[0].body {
        assert_eq!(
            post_kind,
            &PubkyAppPostKind::Long,
            "Tagging an article should report post_kind = Long"
        );
    } else {
        panic!("Expected a TagPost notification, found something else");
    }

    Ok(())
}
