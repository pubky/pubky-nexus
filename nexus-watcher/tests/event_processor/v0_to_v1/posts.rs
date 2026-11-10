//! The posts step: addresses rewritten at the flip, caches rewritten with them, and reruns.

use crate::event_processor::posts::utils::{
    corrupt_post_indexed_at, find_post_details, find_post_uri, pubky_id, restore_post_indexed_at,
    set_post_uri, short_post, short_reply, short_repost, test_user, v1_note, v1_test_user,
};
use crate::event_processor::users::utils::{
    assert_flipped, find_user_uri, set_user_epoch, v1_profile_uri,
};
use crate::event_processor::utils::watcher::{
    generate_post_id, retrieve_and_handle_event_line, HomeserverHashIdPath, WatcherTest,
};
use anyhow::Result;
use chrono::Utc;
use nexus_common::db::graph::Query;
use nexus_common::db::kv::AuthorFilter;
use nexus_common::db::{exec_single_row, fetch_key_from_graph};
use nexus_common::models::event::EventLine;
use nexus_common::models::notification::{Notification, NotificationBody, PostChangedSource};
use nexus_common::models::post::search::PostsByTagSearch;
use nexus_common::models::post::{PostDetails, PostRelationships, PostView, PostsByContentSearch};
use nexus_common::models::user::Epoch;
use nexus_common::types::Pagination;
use nexus_watcher::events::retry::{IndexKey, RetryEvent};
use nexus_watcher::events::v0_to_v1;
use pubky::Keypair;
use pubky_social_specs::legacy_v0::{
    post_uri_builder as v0_post_uri_builder, user_uri_builder as v0_user_uri_builder, PubkyAppTag,
};
use pubky_social_specs::post_uri_builder as v1_post_uri_builder;

/// Whether the shared event log holds `line`.
async fn event_line_stored(line: &str) -> Result<bool> {
    let (lines, _) = EventLine::get_from_index(None, usize::MAX).await?;
    Ok(lines.iter().any(|stored| stored == line))
}

/// Drops a post's `attachments` list, as a post indexed before the list was stored has none.
async fn unset_post_attachments(author_id: &str, post_id: &str) -> Result<()> {
    let query = Query::new(
        "unset_post_attachments",
        "MATCH (:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
        REMOVE p.attachments",
    )
    .param("author_id", author_id)
    .param("post_id", post_id);
    exec_single_row(query).await?;
    Ok(())
}

/// Whether the post's node holds an `attachments` property at all; a missing one reads back as
/// an empty list, so only the graph tells the two apart.
async fn has_attachments_property(author_id: &str, post_id: &str) -> Result<bool> {
    let query = Query::new(
        "has_attachments_property",
        "MATCH (:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
        RETURN p.attachments IS NOT NULL AS present",
    )
    .param("author_id", author_id)
    .param("post_id", post_id);
    Ok(fetch_key_from_graph(query, "present")
        .await?
        .unwrap_or(false))
}

/// What the migration leaves for a post and the posts that point at it, through the API models
/// and the graph: the stored address, the graph details, whether the node holds an attachments
/// list at all, the served details and the served relationships of each child.
async fn snapshot(
    author_id: &str,
    post_ids: &[String],
    children: &[(String, String)],
) -> Result<serde_json::Value> {
    let mut posts = Vec::new();
    for post_id in post_ids {
        let served = PostView::get_by_id(author_id, post_id, None, None, None)
            .await?
            .expect("the post is served");
        posts.push(serde_json::json!({
            "id": post_id,
            "uri": find_post_uri(author_id, post_id).await,
            "graph": find_post_details(author_id, post_id).await?,
            "has_attachments": has_attachments_property(author_id, post_id).await?,
            "served": served.details,
        }));
    }
    let mut relationships = Vec::new();
    for (child_author_id, child_post_id) in children {
        let served = PostRelationships::get_by_id(child_author_id, child_post_id)
            .await?
            .expect("the child's relationships are served");
        relationships.push(serde_json::json!({
            "id": child_post_id,
            "relationships": served,
        }));
    }
    Ok(serde_json::json!({ "posts": posts, "children": relationships }))
}

/// The flip rewrites every post the user authored to its `social/v1` address, a tombstone
/// included, with its content untouched, and the API reads the new address: on the post and on
/// the parent of a reply and of a repost by another user, whose own posts stay at `pubky.app`.
#[tokio_shared_rt::test(shared)]
async fn test_flip_rewrites_the_users_post_addresses() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:Migration:Posts:Author", "migration"),
        )
        .await?;
    let other_kp = Keypair::random();
    let other_id = test
        .create_user(
            &other_kp,
            &test_user("Watcher:Migration:Posts:Other", "migration"),
        )
        .await?;

    let (post_id, post_path) = test
        .create_post(&author_kp, &short_post("Watcher:Migration:Posts:Post"))
        .await?;
    let v0_uri = v0_post_uri_builder(author_id.clone(), post_id.clone());
    let (reply_id, reply_path) = test
        .create_post(
            &other_kp,
            &short_reply("Watcher:Migration:Posts:Reply", v0_uri.clone()),
        )
        .await?;
    let (repost_id, repost_path) = test
        .create_post(
            &other_kp,
            &short_repost("Watcher:Migration:Posts:Repost", v0_uri.clone()),
        )
        .await?;
    // A tombstone: a post deleted while a reply keeps its node
    let (deleted_id, deleted_path) = test
        .create_post(&author_kp, &short_post("Watcher:Migration:Posts:Deleted"))
        .await?;
    let (deleted_reply_id, deleted_reply_path) = test
        .create_post(
            &other_kp,
            &short_reply(
                "Watcher:Migration:Posts:ReplyToDeleted",
                v0_post_uri_builder(author_id.clone(), deleted_id.clone()),
            ),
        )
        .await?;
    test.cleanup_post(&author_kp, &deleted_path).await?;
    assert!(find_post_details(&author_id, &deleted_id).await?.deleted);
    // Served once before the flip, so the caches hold the old address
    let before = PostView::get_by_id(&author_id, &post_id, None, None, None)
        .await?
        .expect("the post is served");
    assert_eq!(before.details.uri, v0_uri);

    let profile_path = test
        .put_v1_profile(
            &author_kp,
            &v1_test_user("Watcher:Migration:Posts:Author:V1", "migration"),
        )
        .await?;
    assert_flipped(&author_id).await;

    let v1_uri = v1_post_uri_builder(author_id.clone(), post_id.clone());
    assert_eq!(
        v1_uri,
        format!("pubky://{author_id}/pub/social/v1/posts/{post_id}")
    );
    assert_eq!(
        find_post_uri(&author_id, &post_id).await,
        Some(v1_uri.clone())
    );
    let served = PostView::get_by_id(&author_id, &post_id, None, None, None)
        .await?
        .expect("the post is served");
    assert_eq!(served.details.uri, v1_uri);
    assert_eq!(served.details.content, "Watcher:Migration:Posts:Post");
    assert_eq!(served.details.indexed_at, before.details.indexed_at);

    let deleted_v1_uri = v1_post_uri_builder(author_id.clone(), deleted_id.clone());
    assert_eq!(
        find_post_uri(&author_id, &deleted_id).await,
        Some(deleted_v1_uri.clone())
    );
    let tombstone = PostDetails::get_by_id(&author_id, &deleted_id)
        .await?
        .expect("the tombstone is served");
    assert!(tombstone.deleted);
    assert_eq!(tombstone.uri, deleted_v1_uri);

    let reply = PostRelationships::get_by_id(&other_id, &reply_id)
        .await?
        .expect("the reply's relationships are served");
    assert_eq!(reply.replied.as_deref(), Some(v1_uri.as_str()));
    let repost = PostRelationships::get_by_id(&other_id, &repost_id)
        .await?
        .expect("the repost's relationships are served");
    assert_eq!(repost.reposted.as_deref(), Some(v1_uri.as_str()));
    let deleted_reply = PostRelationships::get_by_id(&other_id, &deleted_reply_id)
        .await?
        .expect("the reply's relationships are served");
    assert_eq!(
        deleted_reply.replied.as_deref(),
        Some(deleted_v1_uri.as_str())
    );

    // The other user has not migrated: their posts keep their address
    for other_post_id in [&reply_id, &repost_id, &deleted_reply_id] {
        assert_eq!(
            find_post_uri(&other_id, other_post_id).await,
            Some(v0_post_uri_builder(other_id.clone(), other_post_id.clone()))
        );
    }

    // Back to `v0`, so the gate lets the `pubky.app` cleanup through; the v1 DEL is then ignored
    set_user_epoch(&author_id, "v0").await?;
    test.del(&author_kp, &profile_path).await?;
    test.cleanup_post(&other_kp, &reply_path).await?;
    test.cleanup_post(&other_kp, &repost_path).await?;
    test.cleanup_post(&other_kp, &deleted_reply_path).await?;
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&other_kp).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}

/// After the flip, with no read in between, an old post is still in content search and a new
/// label on it, from a user still at `v0`, reaches that label's tag search: its cached details
/// were rewritten, not deleted.
#[tokio_shared_rt::test(shared)]
async fn test_flip_keeps_an_old_post_searchable() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:Migration:Search:Author", "migration"),
        )
        .await?;
    let tagger_kp = Keypair::random();
    test.create_user(
        &tagger_kp,
        &test_user("Watcher:Migration:Search:Tagger", "migration"),
    )
    .await?;

    let token = format!("migrated{}", generate_post_id().to_lowercase());
    let (post_id, post_path) = test
        .create_post(&author_kp, &short_post(format!("Old post {token}")))
        .await?;
    let post_key = format!("{author_id}:{post_id}");

    let profile_path = test
        .put_v1_profile(
            &author_kp,
            &v1_test_user("Watcher:Migration:Search:Author:V1", "migration"),
        )
        .await?;

    let author = pubky_id(&author_id)?;
    let hits =
        PostsByContentSearch::search(&token, Some(AuthorFilter::One(&author)), None, 0, 10).await?;
    assert!(
        hits.iter().any(|hit| hit.post_key == post_key),
        "the old post must stay in content search, got {:?}",
        hits.iter().map(|hit| &hit.post_key).collect::<Vec<_>>()
    );

    let label = format!("m{}", generate_post_id().to_lowercase());
    let tag = PubkyAppTag {
        uri: v0_post_uri_builder(author_id.clone(), post_id.clone()),
        label: label.clone(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();
    test.put(&tagger_kp, &tag_path, &tag).await?;

    let tagged = PostsByTagSearch::get_by_label(&label, None, Pagination::default())
        .await?
        .unwrap_or_default();
    assert!(
        tagged.iter().any(|entry| entry.post_key == post_key),
        "the label's tag search must list the unopened post, got {:?}",
        tagged
            .iter()
            .map(|entry| &entry.post_key)
            .collect::<Vec<_>>()
    );
    test.del(&tagger_kp, &tag_path).await?;
    set_user_epoch(&author_id, "v0").await?;
    test.del(&author_kp, &profile_path).await?;
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&tagger_kp).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}

/// After the flip, a real `social/v1` edit of an old post notifies whoever replied, and deleting
/// a reply notifies the author, each naming the post's new address.
#[tokio_shared_rt::test(shared)]
async fn test_flip_keeps_edit_and_reply_delete_notifications() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:Migration:Notify:Author", "migration"),
        )
        .await?;
    let replier_kp = Keypair::random();
    let replier_id = test
        .create_user(
            &replier_kp,
            &test_user("Watcher:Migration:Notify:Replier", "migration"),
        )
        .await?;

    let (post_id, post_path) = test
        .create_post(&author_kp, &short_post("Watcher:Migration:Notify:Post"))
        .await?;
    let v0_uri = v0_post_uri_builder(author_id.clone(), post_id.clone());
    let (reply_id, reply_path) = test
        .create_post(
            &replier_kp,
            &short_reply("Watcher:Migration:Notify:Reply", v0_uri),
        )
        .await?;

    let profile_path = test
        .put_v1_profile(
            &author_kp,
            &v1_test_user("Watcher:Migration:Notify:Author:V1", "migration"),
        )
        .await?;
    let v1_uri = v1_post_uri_builder(author_id.clone(), post_id.clone());

    // A real edit, through `social/v1`
    let edit_id = generate_post_id();
    let version_path = test
        .put_v1_post_version(
            &author_kp,
            &post_id,
            &edit_id,
            &v1_note("Watcher:Migration:Notify:Edited"),
        )
        .await?;

    let notifications = Notification::get_by_id(&replier_id, Pagination::default()).await?;
    assert_eq!(notifications.len(), 1, "the replier is told of the edit");
    match &notifications[0].body {
        NotificationBody::PostEdited {
            edit_source,
            edited_by,
            edited_uri,
            ..
        } => {
            assert_eq!(edit_source, &PostChangedSource::ReplyParent);
            assert_eq!(edited_by, &author_id);
            assert_eq!(edited_uri, &v1_uri);
        }
        other => panic!("expected a PostEdited notification, got {other:?}"),
    }

    // The replier, still at `v0`, deletes the reply
    test.cleanup_post(&replier_kp, &reply_path).await?;

    let notifications = Notification::get_by_id(&author_id, Pagination::default()).await?;
    assert_eq!(
        notifications.len(),
        2,
        "the author is told of the reply and of its deletion"
    );
    match &notifications[0].body {
        NotificationBody::PostDeleted {
            delete_source,
            deleted_by,
            deleted_uri,
            linked_uri,
            ..
        } => {
            assert_eq!(delete_source, &PostChangedSource::Reply);
            assert_eq!(deleted_by, &replier_id);
            assert_eq!(
                deleted_uri,
                &v0_post_uri_builder(replier_id.clone(), reply_id.clone())
            );
            assert_eq!(linked_uri, &v1_uri);
        }
        other => panic!("expected a PostDeleted notification, got {other:?}"),
    }

    // Handled at `v1`
    test.del(&author_kp, &version_path).await?;
    // Back to `v0`, so the gate lets the `pubky.app` cleanup through; the v1 DEL is then ignored
    set_user_epoch(&author_id, "v0").await?;
    test.del(&author_kp, &profile_path).await?;
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&replier_kp).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}

/// A flip whose migration fails leaves the user at `v0`, with the event queued for retry and
/// nothing stored; the retry finishes it over the posts whose addresses the failed run already
/// rewrote. (The error log is checked where it is written, in the migration's unit tests.)
#[tokio_shared_rt::test(shared)]
async fn test_failed_migration_leaves_v0_and_the_retry_finishes_it() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:Migration:Failed:Author", "migration"),
        )
        .await?;
    let (first_id, first_path) = test
        .create_post(&author_kp, &short_post("Watcher:Migration:Failed:First"))
        .await?;
    let (second_id, second_path) = test
        .create_post(&author_kp, &short_post("Watcher:Migration:Failed:Second"))
        .await?;
    let indexed_at = find_post_details(&author_id, &second_id).await?.indexed_at;

    corrupt_post_indexed_at(&author_id, &second_id).await?;
    let profile_path = test
        .put_v1_profile(
            &author_kp,
            &v1_test_user("Watcher:Migration:Failed:Author:V1", "migration"),
        )
        .await?;

    assert_eq!(Epoch::get_from_graph(&author_id).await?, Epoch::V0);
    let profile_uri = v1_profile_uri(&author_id);
    let line = format!("PUT {profile_uri}");
    assert!(
        RetryEvent::check_index_key(&IndexKey::for_uri(&profile_uri)).await?,
        "the failed flip must be queued for retry"
    );
    assert!(!event_line_stored(&line).await?);
    // The flip step runs before the fetch: a failed one leaves the `pubky.app` profile indexed
    assert_eq!(
        find_user_uri(&author_id).await,
        Some(v0_user_uri_builder(author_id.clone()))
    );
    // The batch's graph write landed before the read failed
    assert_eq!(
        find_post_uri(&author_id, &first_id).await,
        Some(v1_post_uri_builder(author_id.clone(), first_id.clone()))
    );

    // The retry, once the row reads again
    restore_post_indexed_at(&author_id, &second_id, indexed_at).await?;
    let event_handler = test.event_processor_runner.event_handler.clone();
    retrieve_and_handle_event_line(&line, event_handler).await?;

    assert_flipped(&author_id).await;
    assert!(event_line_stored(&line).await?);
    for post_id in [&first_id, &second_id] {
        let expected = v1_post_uri_builder(author_id.clone(), post_id.clone());
        assert_eq!(
            find_post_uri(&author_id, post_id).await,
            Some(expected.clone())
        );
        let served = PostView::get_by_id(&author_id, post_id, None, None, None)
            .await?
            .expect("the post is served");
        assert_eq!(served.details.uri, expected);
    }
    assert_eq!(
        find_post_details(&author_id, &second_id).await?.indexed_at,
        indexed_at
    );

    // Back to `v0`, so the gate lets the `pubky.app` cleanup through; the v1 DEL is then ignored
    set_user_epoch(&author_id, "v0").await?;
    test.del(&author_kp, &profile_path).await?;
    test.cleanup_post(&author_kp, &first_path).await?;
    test.cleanup_post(&author_kp, &second_path).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}

/// Over more than one batch, running the migration again, and running it from a partly migrated
/// state, leaves the same graph and the same served posts and relationships. A post stored with
/// no attachments list, as older data is, keeps none.
#[tokio_shared_rt::test(shared)]
async fn test_migration_rerun_gives_the_same_result() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:Migration:Rerun:Author", "migration"),
        )
        .await?;
    let other_kp = Keypair::random();
    let other_id = test
        .create_user(
            &other_kp,
            &test_user("Watcher:Migration:Rerun:Other", "migration"),
        )
        .await?;

    let mut post_ids = Vec::new();
    let mut post_paths = Vec::new();
    for n in 0..5 {
        let (post_id, post_path) = test
            .create_post(
                &author_kp,
                &short_post(format!("Watcher:Migration:Rerun:Post:{n}")),
            )
            .await?;
        post_ids.push(post_id);
        post_paths.push(post_path);
    }
    let mut children = Vec::new();
    let mut child_paths = Vec::new();
    for (n, parent_id) in post_ids.iter().enumerate().take(3) {
        let parent_uri = v0_post_uri_builder(author_id.clone(), parent_id.clone());
        let post = if n % 2 == 0 {
            short_reply(format!("Watcher:Migration:Rerun:Reply:{n}"), parent_uri)
        } else {
            short_repost(format!("Watcher:Migration:Rerun:Repost:{n}"), parent_uri)
        };
        let (child_id, child_path) = test.create_post(&other_kp, &post).await?;
        children.push((other_id.clone(), child_id));
        child_paths.push(child_path);
    }

    // Older data: a post stored with no attachments list at all
    let bare_id = &post_ids[4];
    unset_post_attachments(&author_id, bare_id).await?;

    let author = pubky_id(&author_id)?;
    let files = v0_to_v1::files::FileMap::load(&author).await?;
    v0_to_v1::posts::migrate(&author, &files, 2).await?;
    let first = snapshot(&author_id, &post_ids, &children).await?;
    for post_id in &post_ids {
        assert_eq!(
            find_post_uri(&author_id, post_id).await,
            Some(v1_post_uri_builder(author_id.clone(), post_id.clone()))
        );
    }
    // The missing list stays missing in the graph, and reads and is served as an empty list; a
    // stored list stays a list
    assert!(!has_attachments_property(&author_id, bare_id).await?);
    assert!(find_post_details(&author_id, bare_id)
        .await?
        .attachments
        .is_empty());
    let served = PostView::get_by_id(&author_id, bare_id, None, None, None)
        .await?
        .expect("the post is served");
    assert!(served.details.attachments.is_empty());
    assert!(has_attachments_property(&author_id, &post_ids[0]).await?);
    for (child_author_id, child_id) in &children {
        let served = PostRelationships::get_by_id(child_author_id, child_id)
            .await?
            .expect("the child's relationships are served");
        let parent = served.replied.or(served.reposted).expect("a parent");
        assert!(parent.contains("/pub/social/v1/posts/"), "{parent}");
    }

    v0_to_v1::posts::migrate(&author, &files, 2).await?;
    assert_eq!(snapshot(&author_id, &post_ids, &children).await?, first);

    // Partly migrated, as a failed run leaves it: some addresses back at `pubky.app`
    for post_id in [&post_ids[1], &post_ids[3]] {
        let v0_uri = v0_post_uri_builder(author_id.clone(), post_id.clone());
        set_post_uri(&author_id, post_id, Some(&v0_uri)).await?;
    }
    assert_ne!(snapshot(&author_id, &post_ids, &children).await?, first);
    v0_to_v1::posts::migrate(&author, &files, 2).await?;
    assert_eq!(snapshot(&author_id, &post_ids, &children).await?, first);

    for child_path in &child_paths {
        test.cleanup_post(&other_kp, child_path).await?;
    }
    for post_path in &post_paths {
        test.cleanup_post(&author_kp, post_path).await?;
    }
    test.cleanup_user(&other_kp).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}
