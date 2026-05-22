//! Regression: a repost of a Collection must NOT add the Collection to
//! POST_TOTAL_ENGAGEMENT. Symmetric INC + DEC gate, same bug class as
//! `reply_to_collection.rs`.

use super::utils::{
    check_member_total_engagement_user_posts, collection_post, short_repost, test_user,
};
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky::Keypair;
use pubky_app_specs::post_uri_builder;

#[tokio_shared_rt::test(shared)]
async fn test_repost_of_collection_does_not_leak_into_engagement() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let author_kp = Keypair::random();
    let author = test_user("CollAuthor", "test_repost_of_collection");
    let author_id = test.create_user(&author_kp, &author).await?;

    let collection = collection_post("Best long reads", None, vec![]);
    let (coll_id, coll_path) = test.create_post(&author_kp, &collection).await?;
    let coll_uri = post_uri_builder(author_id.clone(), coll_id.clone());

    let coll_key: &[&str] = &[&author_id, &coll_id];
    assert!(
        check_member_total_engagement_user_posts(coll_key)
            .await?
            .is_none(),
        "pre-condition: Collection must not be in POST_TOTAL_ENGAGEMENT after PUT"
    );

    let reposter_kp = Keypair::random();
    let reposter = test_user("Reposter", "test_repost_of_collection_reposter");
    test.create_user(&reposter_kp, &reposter).await?;
    let repost = short_repost("ICYMI", coll_uri);
    let (_, repost_path) = test.create_post(&reposter_kp, &repost).await?;

    assert!(
        check_member_total_engagement_user_posts(coll_key)
            .await?
            .is_none(),
        "INC-side bug: repost of Collection added it to POST_TOTAL_ENGAGEMENT (ZINCRBY +1 created the member)"
    );

    test.cleanup_post(&reposter_kp, &repost_path).await?;
    assert!(
        check_member_total_engagement_user_posts(coll_key)
            .await?
            .is_none(),
        "DEL-side bug: repost DEL leaked the Collection into POST_TOTAL_ENGAGEMENT with a negative score (ZINCRBY -1 creates the member if absent)"
    );

    test.cleanup_post(&author_kp, &coll_path).await?;
    test.cleanup_user(&author_kp).await?;
    test.cleanup_user(&reposter_kp).await?;
    Ok(())
}
