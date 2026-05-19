//! Regression: a reply targeting a Collection parent must NOT add the
//! Collection to POST_TOTAL_ENGAGEMENT. The bug class is symmetric:
//!
//! - PUT-side: `increment_score_index_sorted_set` is `ZINCRBY +1`. With no
//!   collection gate, replying to a Collection creates a sorted-set member
//!   for the Collection with score 1, surfacing it in Hot streams.
//!
//! - DEL-side: `decrement_score_index_sorted_set` is `ZINCRBY -1`, with the
//!   same create-on-missing semantics. Even after the PUT-side is gated, a
//!   reply deletion without the symmetric DEL-side gate would create the
//!   Collection member with score -1 — a negative-score leak that surfaces
//!   in default-order Hot stream queries.

use super::utils::{
    check_member_total_engagement_user_posts, collection_post, short_reply, test_user,
};
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky::Keypair;
use pubky_app_specs::post_uri_builder;

#[tokio_shared_rt::test(shared)]
async fn test_reply_to_collection_does_not_leak_into_engagement() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let author_kp = Keypair::random();
    let author = test_user("CollAuthor", "test_reply_to_collection");
    let author_id = test.create_user(&author_kp, &author).await?;

    let collection = collection_post("AI papers", None, vec![]);
    let (coll_id, coll_path) = test.create_post(&author_kp, &collection).await?;
    let coll_uri = post_uri_builder(author_id.clone(), coll_id.clone());

    let coll_key: &[&str] = &[&author_id, &coll_id];
    assert!(
        check_member_total_engagement_user_posts(coll_key)
            .await?
            .is_none(),
        "pre-condition: Collection must not be in POST_TOTAL_ENGAGEMENT after PUT"
    );

    let replier_kp = Keypair::random();
    let replier = test_user("Replier", "test_reply_to_collection_replier");
    test.create_user(&replier_kp, &replier).await?;
    let reply = short_reply("re: AI papers", coll_uri);
    let (_, reply_path) = test.create_post(&replier_kp, &reply).await?;

    assert!(
        check_member_total_engagement_user_posts(coll_key)
            .await?
            .is_none(),
        "INC-side bug: reply to Collection added it to POST_TOTAL_ENGAGEMENT (ZINCRBY +1 created the member)"
    );

    test.cleanup_post(&replier_kp, &reply_path).await?;
    assert!(
        check_member_total_engagement_user_posts(coll_key)
            .await?
            .is_none(),
        "DEL-side bug: reply DEL leaked the Collection into POST_TOTAL_ENGAGEMENT with a negative score (ZINCRBY -1 creates the member if absent)"
    );

    test.cleanup_post(&author_kp, &coll_path).await?;
    test.cleanup_user(&author_kp).await?;
    test.cleanup_user(&replier_kp).await?;
    Ok(())
}
