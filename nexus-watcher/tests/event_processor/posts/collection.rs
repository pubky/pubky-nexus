use anyhow::Result;
use chrono::Utc;
use pubky::Keypair;
use pubky_app_specs::{post_uri_builder, PubkyAppTag};

use super::utils::{
    check_member_global_timeline_user_post, check_member_total_engagement_user_posts,
    check_member_user_post_timeline, collection_post, find_post_counts, find_post_details,
    test_user,
};
use crate::event_processor::tags::utils::{
    check_member_post_tag_global_timeline, check_member_total_engagement_post_tag, find_post_tag,
};
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};

/// PUT a `kind=collection` post and confirm:
/// - the graph node exists with `kind="collection"`,
/// - PostCounts is initialized,
/// - the post is NOT in the global timeline sorted set,
/// - the post is NOT in the per-user timeline sorted set,
/// - the post is NOT in the global engagement sorted set.
///
/// This is the load-bearing assertion for the Phase 3 stream-suppression
/// strategy: collections appear in the graph (so they can be queried via the
/// Cypher fallback with `?kind=collection`) but never enter the Redis index
/// sorted sets that power the default Home / Hot / By-Tag streams.
#[tokio_shared_rt::test(shared)]
async fn test_create_collection_post_indexes_graph_only() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user = test_user("Watcher:Collection:User", "test_collection_post");
    let user_id = test.create_user(&user_kp, &user).await?;

    let post = collection_post(
        "AI papers",
        Some("Best stuff"),
        vec![
            format!("pubky://{user_id}/pub/pubky.app/posts/0034A0X7NJ52A"),
            format!("pubky://{user_id}/pub/pubky.app/posts/0034A0X7NJ52B"),
            format!("pubky://{user_id}/pub/pubky.app/posts/0034A0X7NJ52C"),
        ],
    );

    let (post_id, post_path) = test.create_post(&user_kp, &post).await?;

    // Graph: the collection node exists with kind="collection".
    let post_details = find_post_details(&user_id, &post_id).await?;
    assert_eq!(post_details.id, post_id);
    assert_eq!(
        post_details.kind,
        pubky_app_specs::PubkyAppPostKind::Collection
    );
    assert_eq!(post_details.attachments.as_ref().unwrap().len(), 3);

    // PostCounts initialized at zero.
    let counts = find_post_counts(&user_id, &post_id).await;
    assert_eq!(counts.replies, 0);
    assert_eq!(counts.reposts, 0);
    assert_eq!(counts.tags, 0);

    // Sorted sets must NOT include the collection.
    let global_timeline = check_member_global_timeline_user_post(&user_id, &post_id).await?;
    assert!(
        global_timeline.is_none(),
        "collection must not be in global timeline sorted set"
    );
    let per_user_timeline = check_member_user_post_timeline(&user_id, &post_id).await?;
    assert!(
        per_user_timeline.is_none(),
        "collection must not be in per-user timeline sorted set"
    );
    let engagement = check_member_total_engagement_user_posts(&[&user_id, &post_id]).await?;
    assert!(
        engagement.is_none(),
        "collection must not be in global engagement sorted set"
    );

    test.cleanup_post(&user_kp, &post_path).await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// Edit a `kind=collection` post (rename, add an item) and confirm:
/// - the graph node reflects the edit,
/// - the post still does NOT appear in any of the suppressed sorted sets.
#[tokio_shared_rt::test(shared)]
async fn test_edit_collection_post_keeps_streams_suppressed() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user = test_user("Watcher:Collection:Edit:User", "edit_collection_post");
    let user_id = test.create_user(&user_kp, &user).await?;

    // Create with 3 items.
    let original_attachments = vec![
        format!("pubky://{user_id}/pub/pubky.app/posts/0034A0X7NJ52A"),
        format!("pubky://{user_id}/pub/pubky.app/posts/0034A0X7NJ52B"),
        format!("pubky://{user_id}/pub/pubky.app/posts/0034A0X7NJ52C"),
    ];
    let post = collection_post(
        "Original name",
        Some("Original description"),
        original_attachments.clone(),
    );
    let (post_id, post_path) = test.create_post(&user_kp, &post).await?;

    // Edit: rename + add a 4th item. Re-PUT to the same path.
    let mut edited_attachments = original_attachments.clone();
    edited_attachments.push(format!(
        "pubky://{user_id}/pub/pubky.app/posts/0034A0X7NJ52D"
    ));
    let edited = collection_post(
        "Renamed",
        Some("Updated description"),
        edited_attachments.clone(),
    );
    test.put(&user_kp, &post_path, &edited).await?;

    // Graph reflects the edit.
    let post_details = find_post_details(&user_id, &post_id).await?;
    assert_eq!(post_details.attachments.as_ref().unwrap().len(), 4);

    // Sorted sets still do not contain the collection.
    assert!(
        check_member_global_timeline_user_post(&user_id, &post_id)
            .await?
            .is_none(),
        "edited collection must stay out of global timeline"
    );
    assert!(
        check_member_user_post_timeline(&user_id, &post_id)
            .await?
            .is_none(),
        "edited collection must stay out of per-user timeline"
    );
    assert!(
        check_member_total_engagement_user_posts(&[&user_id, &post_id])
            .await?
            .is_none(),
        "edited collection must stay out of engagement set"
    );

    test.cleanup_post(&user_kp, &post_path).await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// Tagging a Collection — a second user adds a tag to someone else's
/// collection. The Phase-3 invariant: the graph TAGGED edge is created
/// normally (so the tag is discoverable via the post's tag list), but
/// the by-tag *stream* (`PostsByTagSearch`) must NOT include the
/// collection (collections never appear in `?tags=LABEL` results, even
/// when tagged). This is the load-bearing assertion for the
/// `target_post_is_collection` gate in `nexus-watcher/.../tag.rs`.
#[tokio_shared_rt::test(shared)]
async fn test_tag_on_collection_indexes_graph_skips_by_tag_stream() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Author: creates the collection.
    let author_kp = Keypair::random();
    let author = test_user(
        "Watcher:Collection:TagOnCollection:Author",
        "tag_on_collection_author",
    );
    let author_id = test.create_user(&author_kp, &author).await?;

    // Tagger: a different user who will tag the collection.
    let tagger_kp = Keypair::random();
    let tagger = test_user(
        "Watcher:Collection:TagOnCollection:Tagger",
        "tag_on_collection_tagger",
    );
    let _tagger_id = test.create_user(&tagger_kp, &tagger).await?;

    let post = collection_post(
        "Interesting reads",
        Some("Worth bookmarking"),
        vec![format!(
            "pubky://{author_id}/pub/pubky.app/posts/0034A0X7NJ52A"
        )],
    );
    let (post_id, post_path) = test.create_post(&author_kp, &post).await?;

    // Pre-condition: collection is not in the by-tag stream (no tag yet).
    let label = "interesting";
    let post_key: &[&str] = &[&author_id, &post_id];
    assert!(
        check_member_post_tag_global_timeline(post_key, label)
            .await?
            .is_none(),
        "pre-condition: tag should not exist yet"
    );

    // Tagger PUTs the tag.
    let tag = PubkyAppTag {
        uri: post_uri_builder(author_id.to_string(), post_id.clone()),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();
    test.put(&tagger_kp, &tag_path, tag).await?;

    // Assertion 1: the TAGGED edge exists in the graph (normal tag indexing).
    // `find_post_tag`'s Cypher matches the AUTHOR of the tagged post (not the
    // tagger), so pass author_id here.
    let post_tag = find_post_tag(&author_id, &post_id, label).await.unwrap();
    assert!(
        post_tag.is_some(),
        "TAGGED edge must exist in the graph after PUT"
    );

    // Assertion 2: the collection is NOT in the by-tag stream
    // (`PostsByTagSearch` / `TAG_GLOBAL_POST_TIMELINE` sorted set).
    // This is the Phase-3 gate at `tag.rs`: when target_post_is_collection
    // returns true, we skip `PostsByTagSearch::put_to_index`.
    assert!(
        check_member_post_tag_global_timeline(post_key, label)
            .await?
            .is_none(),
        "collection MUST NOT appear in by-tag stream, even when tagged"
    );

    // Assertion 3: the collection is NOT in TAG_GLOBAL_POST_ENGAGEMENT.
    // Without the gate on `PostsByTagSearch::update_index_score`, ZINCRBY
    // would create a member for the collection with score=1, surfacing it on
    // `?tags=LABEL&sorting=total_engagement` queries.
    assert!(
        check_member_total_engagement_post_tag(post_key, label)
            .await?
            .is_none(),
        "collection MUST NOT appear in TAG_GLOBAL_POST_ENGAGEMENT, even when tagged"
    );

    // Assertion 4: the collection is NOT in POST_TOTAL_ENGAGEMENT (the global
    // Hot stream). Without the gate on `PostStream::update_index_score`,
    // ZINCRBY on a non-existent member would create the collection there too,
    // leaking it into `?sorting=total_engagement` queries.
    assert!(
        check_member_total_engagement_user_posts(post_key)
            .await?
            .is_none(),
        "collection MUST NOT appear in POST_TOTAL_ENGAGEMENT, even when tagged"
    );

    // DEL the tag and confirm all four sorted sets stay clean — the decrement
    // path has its own gate so we don't drive any member to a negative score.
    test.del(&tagger_kp, &tag_path).await?;
    assert!(
        check_member_post_tag_global_timeline(post_key, label)
            .await?
            .is_none(),
        "by-tag stream must stay empty after DEL"
    );
    assert!(
        check_member_total_engagement_post_tag(post_key, label)
            .await?
            .is_none(),
        "TAG_GLOBAL_POST_ENGAGEMENT must stay empty after DEL (no negative-score leak)"
    );
    assert!(
        check_member_total_engagement_user_posts(post_key)
            .await?
            .is_none(),
        "POST_TOTAL_ENGAGEMENT must stay empty after DEL (no negative-score leak)"
    );

    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&author_kp).await?;
    test.cleanup_user(&tagger_kp).await?;
    Ok(())
}

/// DEL a `kind=collection` post that has no surviving relationships and
/// confirm the graph node is removed (no orphan sorted-set entries).
#[tokio_shared_rt::test(shared)]
async fn test_delete_collection_post_removes_graph_no_orphans() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user = test_user("Watcher:Collection:Del:User", "del_collection_post");
    let user_id = test.create_user(&user_kp, &user).await?;

    let post = collection_post("Drafts", None, vec![]);
    let (post_id, post_path) = test.create_post(&user_kp, &post).await?;

    // Confirm the collection was created.
    let _ = find_post_details(&user_id, &post_id).await?;

    // Delete and confirm the graph node is gone.
    test.cleanup_post(&user_kp, &post_path).await?;

    let result = find_post_details(&user_id, &post_id).await;
    assert!(
        result.is_err(),
        "deleted collection must not be in the graph"
    );

    // Sorted sets had no entries to begin with — confirm still empty.
    assert!(check_member_global_timeline_user_post(&user_id, &post_id)
        .await?
        .is_none());
    assert!(
        check_member_total_engagement_user_posts(&[&user_id, &post_id])
            .await?
            .is_none()
    );

    test.cleanup_user(&user_kp).await?;
    Ok(())
}
