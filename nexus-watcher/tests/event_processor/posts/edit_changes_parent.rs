use super::utils::{
    check_member_global_timeline_user_post, check_member_post_replies,
    check_member_total_engagement_user_posts, check_member_user_post_timeline, find_post_details,
    short_post, short_reply, short_repost, test_user,
};
use crate::event_processor::users::utils::find_user_counts;
use crate::event_processor::utils::watcher::{retrieve_and_handle_event_line, WatcherTest};
use anyhow::Result;
use nexus_common::models::notification::{Notification, NotificationBody};
use nexus_common::models::post::{PostParent, PostParents};
use nexus_common::types::Pagination;
use nexus_watcher::events::retry::RetryScheduler;
use nexus_watcher::EventProcessorError;
use pubky::Keypair;
use pubky_social_specs::legacy_v0::post_uri_builder;

/// Handles the `PUT` or `DEL` event of `user_id`'s post `post_id` directly, so the test sees
/// the handler's result.
async fn handle(
    test: &WatcherTest,
    event_type: &str,
    user_id: &str,
    post_id: &str,
) -> Result<(), EventProcessorError> {
    let line = format!(
        "{event_type} {}",
        post_uri_builder(user_id.to_string(), post_id.to_string())
    );
    retrieve_and_handle_event_line(&line, test.event_processor_runner.event_handler.clone()).await
}

/// The `(owner_id, post_id)` keys of the post's reply parent and repost embed, from its edges.
async fn edge_keys(
    author_id: &str,
    post_id: &str,
) -> Result<(Option<(String, String)>, Option<(String, String)>)> {
    let parents = PostParents::get_from_graph(author_id, post_id)
        .await?
        .expect("the post is in the graph");
    let key = |parent: Option<PostParent>| {
        parent.map(|parent| (parent.key.owner_id.to_string(), parent.key.resource_id))
    };
    Ok((key(parents.replied), key(parents.reposted)))
}

/// An edit that adds, drops or moves a post's reply parent or repost embed is rejected whole and
/// never retried: the edges, the content and the counts stay as they were created.
#[tokio_shared_rt::test(shared)]
async fn test_edit_that_changes_parent_or_embed_is_rejected_and_not_retried() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:EditChangesParent:Author", "edit"),
        )
        .await?;
    let parent_kp = Keypair::random();
    let parent_author_id = test
        .create_user(
            &parent_kp,
            &test_user("Watcher:EditChangesParent:ParentAuthor", "edit"),
        )
        .await?;

    let (p1_id, _) = test
        .create_post(&parent_kp, &short_post("Watcher:EditChangesParent:P1"))
        .await?;
    let (p2_id, _) = test
        .create_post(&parent_kp, &short_post("Watcher:EditChangesParent:P2"))
        .await?;
    let p1_uri = post_uri_builder(parent_author_id.clone(), p1_id.clone());
    let p2_uri = post_uri_builder(parent_author_id.clone(), p2_id.clone());

    let root = short_post("Watcher:EditChangesParent:Root");
    let (root_id, root_path) = test.create_post(&author_kp, &root).await?;
    let reply = short_reply("Watcher:EditChangesParent:Reply", p1_uri.clone());
    let (reply_id, reply_path) = test.create_post(&author_kp, &reply).await?;

    let counts_before = find_user_counts(&author_id).await;

    // Switch OFF event processing so each edit is handled only by the call below
    test = test.remove_event_processing().await;

    let cases = [
        (
            "root edited to reply",
            &root_id,
            &root_path,
            &root,
            short_reply("Watcher:EditChangesParent:Root:Edited", p1_uri.clone()),
        ),
        (
            "root edited to repost",
            &root_id,
            &root_path,
            &root,
            short_repost("Watcher:EditChangesParent:Root:Edited", p1_uri.clone()),
        ),
        (
            "reply moved to another post",
            &reply_id,
            &reply_path,
            &reply,
            short_reply("Watcher:EditChangesParent:Reply:Edited", p2_uri),
        ),
        (
            "reply edited to drop its parent",
            &reply_id,
            &reply_path,
            &reply,
            short_post("Watcher:EditChangesParent:Reply:Edited"),
        ),
    ];
    for (case, post_id, path, original, edited) in cases {
        let edges_before = edge_keys(&author_id, post_id).await?;

        test.put(&author_kp, path, &edited).await?;
        let err = handle(&test, "PUT", &author_id, post_id)
            .await
            .expect_err("an edit that changes the parent must be rejected");

        assert!(
            matches!(err, EventProcessorError::PostParentChanged(_)),
            "unexpected error ({case}): {err}"
        );
        assert!(
            !RetryScheduler::should_enqueue_related_event(&err),
            "deterministic rejection must not be retried ({case})"
        );

        // GRAPH_OP: the edges and the content are the ones the post was created with
        assert_eq!(
            edge_keys(&author_id, post_id).await?,
            edges_before,
            "{case}"
        );
        assert_eq!(
            find_post_details(&author_id, post_id).await?.content,
            original.content,
            "the whole edit is rejected ({case})"
        );
    }

    // The root is still a root post, and the reply still replies to P1 only
    assert!(check_member_global_timeline_user_post(&author_id, &root_id)
        .await?
        .is_some());
    assert!(
        check_member_post_replies(&parent_author_id, &p1_id, &[&author_id, &root_id])
            .await?
            .is_none()
    );
    assert!(
        check_member_post_replies(&parent_author_id, &p2_id, &[&author_id, &reply_id])
            .await?
            .is_none()
    );
    let counts_after = find_user_counts(&author_id).await;
    assert_eq!(counts_after.posts, counts_before.posts);
    assert_eq!(counts_after.replies, counts_before.replies);

    Ok(())
}

/// A root post whose edit to reply to or repost P was rejected is deleted as the root post it
/// is: it leaves the timelines and the engagement set, the author's counts come back down, P's
/// engagement score doesn't move, and P's author is not told anything was deleted.
#[tokio_shared_rt::test(shared)]
async fn test_delete_after_a_rejected_parent_edit_cleans_up_as_a_root_post() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:EditChangesParent:Del:Author", "edit"),
        )
        .await?;
    let parent_kp = Keypair::random();
    let parent_author_id = test
        .create_user(
            &parent_kp,
            &test_user("Watcher:EditChangesParent:Del:ParentAuthor", "edit"),
        )
        .await?;
    let (p_id, _) = test
        .create_post(&parent_kp, &short_post("Watcher:EditChangesParent:Del:P"))
        .await?;
    let p_uri = post_uri_builder(parent_author_id.clone(), p_id.clone());

    let (to_reply_id, to_reply_path) = test
        .create_post(
            &author_kp,
            &short_post("Watcher:EditChangesParent:Del:ToReply"),
        )
        .await?;
    let (to_repost_id, to_repost_path) = test
        .create_post(
            &author_kp,
            &short_post("Watcher:EditChangesParent:Del:ToRepost"),
        )
        .await?;

    let p_engagement_before =
        check_member_total_engagement_user_posts(&[&parent_author_id, &p_id]).await?;

    // Switch OFF event processing so each event is handled only by the calls below
    test = test.remove_event_processing().await;

    let edits = [
        (
            "reply",
            &to_reply_id,
            &to_reply_path,
            short_reply(
                "Watcher:EditChangesParent:Del:ToReply:Edited",
                p_uri.clone(),
            ),
        ),
        (
            "repost",
            &to_repost_id,
            &to_repost_path,
            short_repost("Watcher:EditChangesParent:Del:ToRepost:Edited", p_uri),
        ),
    ];
    for (case, post_id, path, edited) in edits {
        test.put(&author_kp, path, &edited).await?;
        let err = handle(&test, "PUT", &author_id, post_id)
            .await
            .expect_err("an edit that adds a parent must be rejected");
        assert!(matches!(err, EventProcessorError::PostParentChanged(_)));

        test.del(&author_kp, path).await?;
        handle(&test, "DEL", &author_id, post_id).await?;

        // CACHE_OP: deleted as a root post
        assert!(
            check_member_global_timeline_user_post(&author_id, post_id)
                .await?
                .is_none(),
            "global timeline ({case})"
        );
        assert!(
            check_member_user_post_timeline(&author_id, post_id)
                .await?
                .is_none(),
            "user stream ({case})"
        );
        assert!(
            check_member_total_engagement_user_posts(&[&author_id, post_id])
                .await?
                .is_none(),
            "engagement set ({case})"
        );
        // GRAPH_OP: the post is gone
        assert!(
            find_post_details(&author_id, post_id).await.is_err(),
            "{case}"
        );
    }

    let counts = find_user_counts(&author_id).await;
    assert_eq!(counts.posts, 0);
    assert_eq!(counts.replies, 0);
    assert_eq!(
        check_member_total_engagement_user_posts(&[&parent_author_id, &p_id]).await?,
        p_engagement_before,
        "P's engagement score must not move"
    );
    let deleted = Notification::get_by_id(&parent_author_id, Pagination::default())
        .await?
        .into_iter()
        .filter(|notification| matches!(notification.body, NotificationBody::PostDeleted { .. }))
        .count();
    assert_eq!(
        deleted, 0,
        "P's author must not be told a reply or repost was deleted"
    );

    Ok(())
}

/// An edit that names the same parent, however it is written, or a parent that is not a post,
/// is accepted: only a change to a post edge is rejected.
#[tokio_shared_rt::test(shared)]
async fn test_edit_that_keeps_the_parent_edges_is_accepted() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:EditChangesParent:Keep:Author", "edit"),
        )
        .await?;
    let (p_id, _) = test
        .create_post(&author_kp, &short_post("Watcher:EditChangesParent:Keep:P"))
        .await?;
    let p_uri = post_uri_builder(author_id.clone(), p_id.clone());

    let (reply_id, reply_path) = test
        .create_post(
            &author_kp,
            &short_reply("Watcher:EditChangesParent:Keep:Reply", p_uri.clone()),
        )
        .await?;
    let (url_reply_id, url_reply_path) = test
        .create_post(
            &author_kp,
            &short_reply(
                "Watcher:EditChangesParent:Keep:UrlReply",
                "https://example.com/a".to_string(),
            ),
        )
        .await?;

    // Switch OFF event processing so each edit is handled only by the call below
    test = test.remove_event_processing().await;

    let cases = [
        (
            "same parent written non-canonically",
            &reply_id,
            &reply_path,
            short_reply(
                "Watcher:EditChangesParent:Keep:Reply:Edited",
                format!("{p_uri}/extra"),
            ),
            (Some((author_id.clone(), p_id.clone())), None),
        ),
        (
            "a URL parent changed to another URL",
            &url_reply_id,
            &url_reply_path,
            short_reply(
                "Watcher:EditChangesParent:Keep:UrlReply:Edited",
                "https://example.com/b".to_string(),
            ),
            (None, None),
        ),
    ];
    for (case, post_id, path, edited, edges) in cases {
        test.put(&author_kp, path, &edited).await?;
        handle(&test, "PUT", &author_id, post_id)
            .await
            .unwrap_or_else(|e| panic!("the edit must be accepted ({case}): {e}"));

        assert_eq!(
            find_post_details(&author_id, post_id).await?.content,
            edited.content,
            "{case}"
        );
        assert_eq!(edge_keys(&author_id, post_id).await?, edges, "{case}");
    }

    Ok(())
}

/// A deleted repost that another post replies to becomes a tombstone, and keeps its embed: the
/// tombstone names the same parents as the post, so the edit check lets it through.
#[tokio_shared_rt::test(shared)]
async fn test_tombstone_of_a_repost_keeps_its_embed() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:EditChangesParent:Tombstone:Author", "edit"),
        )
        .await?;
    let other_kp = Keypair::random();
    let other_id = test
        .create_user(
            &other_kp,
            &test_user("Watcher:EditChangesParent:Tombstone:Other", "edit"),
        )
        .await?;
    let (p_id, _) = test
        .create_post(
            &other_kp,
            &short_post("Watcher:EditChangesParent:Tombstone:P"),
        )
        .await?;
    let (repost_id, repost_path) = test
        .create_post(
            &author_kp,
            &short_repost(
                "Watcher:EditChangesParent:Tombstone:Repost",
                post_uri_builder(other_id.clone(), p_id.clone()),
            ),
        )
        .await?;
    // A reply to the repost keeps its node alive on delete
    test.create_post(
        &other_kp,
        &short_reply(
            "Watcher:EditChangesParent:Tombstone:Reply",
            post_uri_builder(author_id.clone(), repost_id.clone()),
        ),
    )
    .await?;

    // Switch OFF event processing so the delete is handled only by the call below
    test = test.remove_event_processing().await;

    test.del(&author_kp, &repost_path).await?;
    handle(&test, "DEL", &author_id, &repost_id).await?;

    let details = find_post_details(&author_id, &repost_id).await?;
    assert!(details.deleted, "the repost is a tombstone");
    assert_eq!(
        edge_keys(&author_id, &repost_id).await?,
        (None, Some((other_id, p_id))),
        "the tombstone keeps its REPOSTED edge"
    );

    Ok(())
}
