use super::utils::{
    check_member_post_replies, find_post_details, set_post_uri, short_post, test_user,
};
use crate::event_processor::utils::watcher::{generate_post_id, HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::notification::{Notification, NotificationBody, PostChangedSource};
use nexus_common::models::post::PostRelationships;
use nexus_common::models::tag::view::TagView;
use nexus_common::types::Pagination;
use pubky::Keypair;
use pubky_social_specs::legacy_v0::traits::HashId;
use pubky_social_specs::legacy_v0::{
    post_uri_builder, PubkyAppPostEmbed, PubkyAppPostKind, PubkyAppTag,
};

async fn notifications_of(user_id: &str) -> Vec<NotificationBody> {
    Notification::get_by_id(user_id, Pagination::default())
        .await
        .unwrap()
        .into_iter()
        .map(|notification| notification.body)
        .collect()
}

/// A stored address that differs from the built `pubky.app` one.
fn stored_uri(author_id: &str, post_id: &str) -> String {
    format!("pubky://{author_id}/pub/social/v1/posts/{post_id}")
}

/// The tag view of a tag on a post returns the target post's stored `uri`.
#[tokio_shared_rt::test(shared)]
async fn test_tag_view_returns_target_post_stored_uri() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:UriReaders:TagView:Author", "uri"),
        )
        .await?;
    let tagger_kp = Keypair::random();
    let tagger_id = test
        .create_user(
            &tagger_kp,
            &test_user("Watcher:UriReaders:TagView:Tagger", "uri"),
        )
        .await?;
    let (post_id, post_path) = test
        .create_post(&author_kp, &short_post("Watcher:UriReaders:TagView:Post"))
        .await?;

    let tag = PubkyAppTag {
        uri: post_uri_builder(author_id.clone(), post_id.clone()),
        label: "uri-tag-view".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_id = tag.create_id();
    let tag_path = tag.hs_path();
    test.put(&tagger_kp, &tag_path, tag).await?;

    let stored = stored_uri(&author_id, &post_id);
    set_post_uri(&author_id, &post_id, Some(&stored)).await?;

    let view = TagView::get_by_tagger_and_id(&tagger_id, &tag_id)
        .await?
        .expect("tag view");
    assert_eq!(view.uri, stored);

    test.del(&tagger_kp, &tag_path).await?;
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&tagger_kp).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}

/// Reply and repost parents read from the graph come from the parent's stored `uri`.
#[tokio_shared_rt::test(shared)]
async fn test_relationship_parents_return_parent_stored_uri() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:UriReaders:Rel:Author", "uri"),
        )
        .await?;
    let (post_id, post_path) = test
        .create_post(&author_kp, &short_post("Watcher:UriReaders:Rel:Post"))
        .await?;
    let parent_uri = post_uri_builder(author_id.clone(), post_id.clone());

    let mut reply = short_post("Watcher:UriReaders:Rel:Reply");
    reply.parent = Some(parent_uri.clone());
    let (reply_id, reply_path) = test.create_post(&author_kp, &reply).await?;

    let mut repost = short_post("Watcher:UriReaders:Rel:Repost");
    repost.embed = Some(PubkyAppPostEmbed {
        kind: PubkyAppPostKind::Short,
        uri: parent_uri,
    });
    let (repost_id, repost_path) = test.create_post(&author_kp, &repost).await?;

    // Store a non-v0 address on the parent, other than the one the reply and repost wrote
    let stored = stored_uri(&author_id, &generate_post_id());
    set_post_uri(&author_id, &post_id, Some(&stored)).await?;

    let replied = PostRelationships::get_from_graph(&author_id, &reply_id)
        .await?
        .and_then(|rel| rel.replied)
        .expect("reply parent");
    assert_eq!(replied, stored);

    let reposted = PostRelationships::get_from_graph(&author_id, &repost_id)
        .await?
        .and_then(|rel| rel.reposted)
        .expect("repost parent");
    assert_eq!(reposted, stored);

    test.cleanup_post(&author_kp, &repost_path).await?;
    test.cleanup_post(&author_kp, &reply_path).await?;
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}

/// A reply and a repost whose parent address is written non-canonically, but still names the
/// post, store the parent's stored `uri` in the Redis relationships and the notifications.
#[tokio_shared_rt::test(shared)]
async fn test_put_parents_written_non_canonically_store_parent_stored_uri() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:UriReaders:PutRel:Author", "uri"),
        )
        .await?;
    let replier_kp = Keypair::random();
    let replier_id = test
        .create_user(
            &replier_kp,
            &test_user("Watcher:UriReaders:PutRel:Replier", "uri"),
        )
        .await?;
    let (post_id, post_path) = test
        .create_post(&author_kp, &short_post("Watcher:UriReaders:PutRel:Post"))
        .await?;
    let stored = post_uri_builder(author_id.clone(), post_id.clone());
    // A trailing segment still resolves to the post
    let written = format!("{stored}/extra");

    let mut reply = short_post("Watcher:UriReaders:PutRel:Reply");
    reply.parent = Some(written.clone());
    let (reply_id, reply_path) = test.create_post(&replier_kp, &reply).await?;

    let mut repost = short_post("Watcher:UriReaders:PutRel:Repost");
    repost.embed = Some(PubkyAppPostEmbed {
        kind: PubkyAppPostKind::Short,
        uri: written,
    });
    let (repost_id, repost_path) = test.create_post(&replier_kp, &repost).await?;

    let replied = PostRelationships::get_from_index(&replier_id, &reply_id)
        .await?
        .and_then(|rel| rel.replied);
    assert_eq!(replied.as_ref(), Some(&stored), "Redis reply parent");
    let reposted = PostRelationships::get_from_index(&replier_id, &repost_id)
        .await?
        .and_then(|rel| rel.reposted);
    assert_eq!(reposted.as_ref(), Some(&stored), "Redis repost parent");

    let mut parent_uris: Vec<String> = notifications_of(&author_id)
        .await
        .into_iter()
        .filter_map(|body| match body {
            NotificationBody::Reply {
                parent_post_uri, ..
            } => Some(parent_post_uri),
            NotificationBody::Repost { embed_uri, .. } => Some(embed_uri),
            _ => None,
        })
        .collect();
    parent_uris.sort();
    assert_eq!(parent_uris, vec![stored.clone(), stored.clone()]);

    // An edit that keeps the parent as written links the parent by its stored `uri` too
    reply.content = "Watcher:UriReaders:PutRel:Reply:Edited".to_string();
    test.put(&replier_kp, &reply_path, &reply).await?;
    let edited: Vec<String> = notifications_of(&author_id)
        .await
        .into_iter()
        .filter_map(|body| match body {
            NotificationBody::PostEdited {
                edit_source: PostChangedSource::Reply,
                linked_uri,
                ..
            } => Some(linked_uri),
            _ => None,
        })
        .collect();
    assert_eq!(edited, vec![stored]);

    test.cleanup_post(&replier_kp, &repost_path).await?;
    test.cleanup_post(&replier_kp, &reply_path).await?;
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&replier_kp).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}

/// An UntagPost notification carries the untagged post's stored `uri`.
#[tokio_shared_rt::test(shared)]
async fn test_untag_notification_returns_post_stored_uri() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:UriReaders:Untag:Author", "uri"),
        )
        .await?;
    let tagger_kp = Keypair::random();
    test.create_user(
        &tagger_kp,
        &test_user("Watcher:UriReaders:Untag:Tagger", "uri"),
    )
    .await?;
    let (post_id, post_path) = test
        .create_post(&author_kp, &short_post("Watcher:UriReaders:Untag:Post"))
        .await?;

    let tag = PubkyAppTag {
        uri: post_uri_builder(author_id.clone(), post_id.clone()),
        label: "uri-untag".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();
    test.put(&tagger_kp, &tag_path, tag).await?;

    let stored = stored_uri(&author_id, &post_id);
    set_post_uri(&author_id, &post_id, Some(&stored)).await?;

    test.del(&tagger_kp, &tag_path).await?;

    let untag_uris: Vec<String> = notifications_of(&author_id)
        .await
        .into_iter()
        .filter_map(|body| match body {
            NotificationBody::UntagPost { post_uri, .. } => Some(post_uri),
            _ => None,
        })
        .collect();
    assert_eq!(untag_uris, vec![stored]);

    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&tagger_kp).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}

/// When a post is edited, the `linked_uri` sent to repliers and reposters is their own post's stored `uri`.
#[tokio_shared_rt::test(shared)]
async fn test_edit_notification_links_interactor_post_stored_uri() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:UriReaders:Edit:Author", "uri"),
        )
        .await?;
    let replier_kp = Keypair::random();
    let replier_id = test
        .create_user(
            &replier_kp,
            &test_user("Watcher:UriReaders:Edit:Replier", "uri"),
        )
        .await?;
    let reposter_kp = Keypair::random();
    let reposter_id = test
        .create_user(
            &reposter_kp,
            &test_user("Watcher:UriReaders:Edit:Reposter", "uri"),
        )
        .await?;

    let (post_id, post_path) = test
        .create_post(&author_kp, &short_post("Watcher:UriReaders:Edit:Post"))
        .await?;
    let parent_uri = post_uri_builder(author_id.clone(), post_id.clone());

    let mut reply = short_post("Watcher:UriReaders:Edit:Reply");
    reply.parent = Some(parent_uri.clone());
    let (reply_id, reply_path) = test.create_post(&replier_kp, &reply).await?;

    let mut repost = short_post("Watcher:UriReaders:Edit:Repost");
    repost.embed = Some(PubkyAppPostEmbed {
        kind: PubkyAppPostKind::Short,
        uri: parent_uri,
    });
    let (repost_id, repost_path) = test.create_post(&reposter_kp, &repost).await?;

    let reply_stored = stored_uri(&replier_id, &reply_id);
    set_post_uri(&replier_id, &reply_id, Some(&reply_stored)).await?;
    let repost_stored = stored_uri(&reposter_id, &repost_id);
    set_post_uri(&reposter_id, &repost_id, Some(&repost_stored)).await?;

    test.put(
        &author_kp,
        &post_path,
        short_post("Watcher:UriReaders:Edit:Edited"),
    )
    .await?;

    let edited_links = |bodies: Vec<NotificationBody>| -> Vec<(PostChangedSource, String)> {
        bodies
            .into_iter()
            .filter_map(|body| match body {
                NotificationBody::PostEdited {
                    edit_source,
                    linked_uri,
                    ..
                } => Some((edit_source, linked_uri)),
                _ => None,
            })
            .collect()
    };
    assert_eq!(
        edited_links(notifications_of(&replier_id).await),
        vec![(PostChangedSource::ReplyParent, reply_stored)]
    );
    assert_eq!(
        edited_links(notifications_of(&reposter_id).await),
        vec![(PostChangedSource::RepostEmbed, repost_stored)]
    );

    test.cleanup_post(&reposter_kp, &repost_path).await?;
    test.cleanup_post(&replier_kp, &reply_path).await?;
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&reposter_kp).await?;
    test.cleanup_user(&replier_kp).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}

/// When a reply with no edges is deleted, the parent's author is told the reply's stored `uri`.
#[tokio_shared_rt::test(shared)]
async fn test_reply_delete_notification_carries_reply_stored_uri() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:UriReaders:Del:Author", "uri"),
        )
        .await?;
    let replier_kp = Keypair::random();
    let replier_id = test
        .create_user(
            &replier_kp,
            &test_user("Watcher:UriReaders:Del:Replier", "uri"),
        )
        .await?;

    let (post_id, post_path) = test
        .create_post(&author_kp, &short_post("Watcher:UriReaders:Del:Post"))
        .await?;
    let mut reply = short_post("Watcher:UriReaders:Del:Reply");
    reply.parent = Some(post_uri_builder(author_id.clone(), post_id.clone()));
    let (reply_id, reply_path) = test.create_post(&replier_kp, &reply).await?;

    let reply_stored = stored_uri(&replier_id, &reply_id);
    set_post_uri(&replier_id, &reply_id, Some(&reply_stored)).await?;

    test.cleanup_post(&replier_kp, &reply_path).await?;

    let deleted: Vec<(PostChangedSource, String)> = notifications_of(&author_id)
        .await
        .into_iter()
        .filter_map(|body| match body {
            NotificationBody::PostDeleted {
                delete_source,
                deleted_uri,
                ..
            } => Some((delete_source, deleted_uri)),
            _ => None,
        })
        .collect();
    assert_eq!(deleted, vec![(PostChangedSource::Reply, reply_stored)]);

    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&replier_kp).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}

/// Deleting a reply whose parent is stored under a `social/v1` address finds the parent by the
/// key on its `REPLIED` edge, not by parsing that address, even on a retry after the reply's
/// relationships index entry is gone.
#[tokio_shared_rt::test(shared)]
async fn test_reply_delete_finds_parent_stored_under_another_address() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:UriReaders:DelParent:Author", "uri"),
        )
        .await?;
    let replier_kp = Keypair::random();
    let replier_id = test
        .create_user(
            &replier_kp,
            &test_user("Watcher:UriReaders:DelParent:Replier", "uri"),
        )
        .await?;

    let (post_id, post_path) = test
        .create_post(&author_kp, &short_post("Watcher:UriReaders:DelParent:Post"))
        .await?;
    let mut reply = short_post("Watcher:UriReaders:DelParent:Reply");
    reply.parent = Some(post_uri_builder(author_id.clone(), post_id.clone()));
    let (reply_id, reply_path) = test.create_post(&replier_kp, &reply).await?;
    let reply_key = [replier_id.as_str(), reply_id.as_str()];
    assert!(check_member_post_replies(&author_id, &post_id, &reply_key)
        .await?
        .is_some());

    set_post_uri(
        &author_id,
        &post_id,
        Some(&stored_uri(&author_id, &post_id)),
    )
    .await?;
    // As on a retry: a first attempt removed the gate, then failed
    PostRelationships::delete(&replier_id, &reply_id).await?;

    test.cleanup_post(&replier_kp, &reply_path).await?;

    // CACHE_OP: the reply left the parent's reply set
    assert!(check_member_post_replies(&author_id, &post_id, &reply_key)
        .await?
        .is_none());
    // GRAPH_OP: the reply is gone
    assert!(find_post_details(&replier_id, &reply_id).await.is_err());

    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&replier_kp).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}

/// When a post with a reply is deleted, its tombstone keeps the stored `uri`, and the replier is
/// told both the deleted post's and their reply's stored `uri`.
#[tokio_shared_rt::test(shared)]
async fn test_tombstone_delete_notification_carries_stored_uris() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:UriReaders:Tombstone:Author", "uri"),
        )
        .await?;
    let replier_kp = Keypair::random();
    let replier_id = test
        .create_user(
            &replier_kp,
            &test_user("Watcher:UriReaders:Tombstone:Replier", "uri"),
        )
        .await?;

    let (post_id, post_path) = test
        .create_post(&author_kp, &short_post("Watcher:UriReaders:Tombstone:Post"))
        .await?;
    let mut reply = short_post("Watcher:UriReaders:Tombstone:Reply");
    reply.parent = Some(post_uri_builder(author_id.clone(), post_id.clone()));
    let (reply_id, reply_path) = test.create_post(&replier_kp, &reply).await?;

    let post_stored = stored_uri(&author_id, &post_id);
    set_post_uri(&author_id, &post_id, Some(&post_stored)).await?;
    let reply_stored = stored_uri(&replier_id, &reply_id);
    set_post_uri(&replier_id, &reply_id, Some(&reply_stored)).await?;

    // The reply keeps an edge on the post, so the DEL leaves a tombstone
    test.cleanup_post(&author_kp, &post_path).await?;

    let deleted: Vec<(PostChangedSource, String, String)> = notifications_of(&replier_id)
        .await
        .into_iter()
        .filter_map(|body| match body {
            NotificationBody::PostDeleted {
                delete_source,
                deleted_uri,
                linked_uri,
                ..
            } => Some((delete_source, deleted_uri, linked_uri)),
            _ => None,
        })
        .collect();
    assert_eq!(
        deleted,
        vec![(PostChangedSource::ReplyParent, post_stored, reply_stored)]
    );

    test.cleanup_post(&replier_kp, &reply_path).await?;
    test.cleanup_user(&replier_kp).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}
