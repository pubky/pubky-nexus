//! Shared helpers for post idempotency recovery tests.
//!
//! The sibling submodules (`put`, `del`) use these simulators to drive the
//! handlers through partial-failure windows and then assert the recovered
//! state is correct.

use crate::event_processor::posts::utils::{
    check_member_global_timeline_user_post, check_member_total_engagement_user_posts,
    check_member_user_post_timeline, pubky_id,
};
use anyhow::Result;
use nexus_common::db::graph::Query;
use nexus_common::db::{exec_single_row, RedisOps};
use nexus_common::models::post::{PostCounts, PostDetails, PostRelationships, PostStream};
use nexus_common::models::user::UserCounts;

mod del;
mod put;

// ---------------------------------------------------------------------------
// Idempotency-test partial-failure simulators
// ---------------------------------------------------------------------------

/// Simulate the partial-failure window of a `sync_put` for a ROOT post:
/// the graph still holds the post but every Redis index/sorted-set entry
/// has been wiped. Caller must NOT also wipe `UserCounts` — the failed
/// attempt is assumed to have already incremented it.
pub(super) async fn simulate_partial_put_failure_root(user_id: &str, post_id: &str) -> Result<()> {
    let post_key: &[&str] = &[user_id, post_id];
    PostDetails::remove_from_index_multiple_json(&[post_key]).await?;
    PostRelationships::remove_from_index_multiple_json(&[post_key]).await?;
    PostCounts::remove_from_index_multiple_json(&[post_key]).await?;
    PostStream::remove_from_timeline_sorted_set(user_id, post_id).await?;
    PostStream::remove_from_per_user_sorted_set(user_id, post_id).await?;
    PostStream::delete_from_engagement_sorted_set(user_id, post_id).await?;
    Ok(())
}

/// Simulate the partial-failure window of a `sync_put` for a REPLY post:
/// in addition to the root wipes, also clear the parent's post-reply
/// sorted set and the author's replies-per-user sorted set (the writes
/// `PostDetails::put_to_index` performs in the `Some(parent)` branch).
///
/// `parent_post_key` is `[parent_author_id, parent_post_id]`.
pub(super) async fn simulate_partial_put_failure_reply(
    author_id: &str,
    post_id: &str,
    parent_post_key: &[&str; 2],
) -> Result<()> {
    let post_key: &[&str] = &[author_id, post_id];
    PostDetails::remove_from_index_multiple_json(&[post_key]).await?;
    PostRelationships::remove_from_index_multiple_json(&[post_key]).await?;
    PostCounts::remove_from_index_multiple_json(&[post_key]).await?;
    PostStream::remove_from_post_reply_sorted_set(parent_post_key, author_id, post_id).await?;
    PostStream::remove_from_replies_per_user_sorted_set(author_id, post_id).await?;
    Ok(())
}

/// Simulate the partial-failure window of a graph-last `sync_del` for a
/// ROOT post: every Redis cleanup step succeeded but the graph delete
/// failed. The gate (`PostRelationships`) is removed atomically, the
/// author's `posts` count was decremented, and the JSON indexes are gone
/// — only the graph node remains.
pub(super) async fn simulate_partial_del_cleanup_root(user_id: &str, post_id: &str) -> Result<()> {
    let post_key: &[&str] = &[user_id, post_id];
    PostRelationships::remove_from_index_multiple_json(&[post_key]).await?;
    UserCounts::decrement(&pubky_id(user_id)?, "posts", None).await?;
    PostDetails::delete_from_index(user_id, post_id, None).await?;
    PostCounts::delete(user_id, post_id, true).await?;
    Ok(())
}

/// Whether a child post in a del-cleanup simulation is a reply or a repost.
/// The two cases share most cleanup steps but differ in: parent-count field,
/// `remove_from_feeds` flag for `PostCounts::delete`, the parent tuple passed
/// to `PostDetails::delete_from_index`, and whether to also decrement the
/// author's `replies` counter.
pub(super) enum ChildKind {
    Reply,
    Repost,
}

/// Simulate the partial-failure window of a graph-last `sync_del` for a
/// CHILD post (reply or repost): every Redis cleanup step completed but the
/// graph delete failed. Covers the scenario-specific writes (parent
/// reply/repost count decrement, parent engagement decrement, and — for
/// replies — the author's `replies` counter decrement).
pub(super) async fn simulate_partial_del_cleanup_child(
    author_id: &str,
    post_id: &str,
    parent_author_id: &str,
    parent_post_id: &str,
    kind: ChildKind,
) -> Result<()> {
    let child_key: &[&str] = &[author_id, post_id];
    let parent_key: &[&str] = &[parent_author_id, parent_post_id];
    let author_pubky = pubky_id(author_id)?;

    // Gate removed atomically.
    PostRelationships::remove_from_index_multiple_json(&[child_key]).await?;
    // Author's post count decremented.
    UserCounts::decrement(&author_pubky, "posts", None).await?;

    let (parent_field, remove_from_feeds, delete_parent_tuple) = match kind {
        ChildKind::Reply => {
            // Replies also decrement the author's `replies` counter.
            UserCounts::decrement(&author_pubky, "replies", None).await?;
            (
                "replies",
                false,
                Some((parent_author_id.to_string(), parent_post_id.to_string())),
            )
        }
        ChildKind::Repost => ("reposts", true, None),
    };

    // Parent's reply/repost count decremented.
    PostCounts::decrement_index_field(parent_key, parent_field, None).await?;
    // Parent engagement score decremented (parent is a root post).
    PostStream::decrement_score_index_sorted_set(
        &nexus_common::models::post::POST_TOTAL_ENGAGEMENT_KEY_PARTS,
        parent_key,
    )
    .await?;
    // PostCounts / PostDetails removed for the child.
    PostCounts::delete(author_id, post_id, remove_from_feeds).await?;
    PostDetails::delete_from_index(author_id, post_id, delete_parent_tuple).await?;
    Ok(())
}

/// Delete the `MENTIONED` graph edge for a given (author, post, mentioned)
/// triple, simulating a mid-mention-loop crash where the `MERGE` never
/// committed.
pub(super) async fn delete_mention_edge(
    author_id: &str,
    post_id: &str,
    mentioned_id: &str,
) -> Result<()> {
    let q = Query::new(
        "test_delete_mention_edge",
        "MATCH (u:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
         MATCH (p)-[m:MENTIONED]->(:User {id: $mentioned_id})
         DELETE m",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
    .param("mentioned_id", mentioned_id);
    exec_single_row(q).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Recovery-state assertions
// ---------------------------------------------------------------------------

/// Assert that all three post JSON indexes (`PostDetails`, `PostRelationships`,
/// `PostCounts`) and all three root-post sorted-set memberships (global
/// timeline, per-user timeline, total engagement) are present — i.e. the
/// full root-post indexed state has been rebuilt.
pub(super) async fn assert_root_post_fully_indexed(user_id: &str, post_id: &str) -> Result<()> {
    let post_key: &[&str] = &[user_id, post_id];
    assert!(
        PostDetails::get_from_index(user_id, post_id)
            .await?
            .is_some(),
        "PostDetails missing for {user_id}:{post_id}"
    );
    assert!(
        PostRelationships::get_from_index(user_id, post_id)
            .await?
            .is_some(),
        "PostRelationships missing for {user_id}:{post_id}"
    );
    assert!(
        PostCounts::get_from_index(user_id, post_id)
            .await?
            .is_some(),
        "PostCounts missing for {user_id}:{post_id}"
    );
    assert!(
        check_member_global_timeline_user_post(user_id, post_id)
            .await?
            .is_some(),
        "global timeline membership missing for {user_id}:{post_id}"
    );
    assert!(
        check_member_user_post_timeline(user_id, post_id)
            .await?
            .is_some(),
        "per-user timeline membership missing for {user_id}:{post_id}"
    );
    assert!(
        check_member_total_engagement_user_posts(post_key)
            .await?
            .is_some(),
        "total-engagement membership missing for {user_id}:{post_id}"
    );
    Ok(())
}
