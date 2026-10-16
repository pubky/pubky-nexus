//! Writes to the sorted sets every viewer shares go through the trust gate: an
//! unranked author's posts stay out of the global timeline and engagement sets
//! and the tag sets. The sets scoped to them are untouched.
//!
//! `WatcherTest::create_user` ranks the users it creates; each test here
//! unranks the authors it needs hidden. No other watcher test rebuilds the
//! ranking, so the change holds for the test's duration.
use super::utils::{
    check_member_global_timeline_user_post, check_member_total_engagement_user_posts,
    check_member_user_post_timeline, short_post, test_user,
};
use crate::event_processor::tags::utils::{
    check_member_post_tag_global_timeline, check_member_total_engagement_post_tag,
};
use crate::event_processor::utils::watcher::{unrank_user, HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use pubky::Keypair;
use pubky_app_specs::{post_uri_builder, PubkyAppPost, PubkyAppTag};

const BIO: &str = "trust gate";

#[tokio_shared_rt::test(shared)]
async fn test_unranked_posts_stay_out_of_shared_sets() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let label = "watchergatelabel";

    let ranked_kp = Keypair::random();
    let ranked_id = test
        .create_user(&ranked_kp, &test_user("Watcher:Gate:Ranked", BIO))
        .await?;
    let unranked_kp = Keypair::random();
    let unranked_id = test
        .create_user(&unranked_kp, &test_user("Watcher:Gate:Unranked", BIO))
        .await?;
    unrank_user(&unranked_id).await?;
    let fan_kp = Keypair::random();
    test.create_user(&fan_kp, &test_user("Watcher:Gate:Fan", BIO))
        .await?;

    let (ranked_post, _) = test
        .create_post(&ranked_kp, &short_post("Watcher:Gate:RankedPost"))
        .await?;
    let (unranked_post, _) = test
        .create_post(&unranked_kp, &short_post("Watcher:Gate:UnrankedPost"))
        .await?;

    // A tag and a reply on each post: the engagement writes must not create a
    // hidden post either.
    for (author_id, post_id) in [(&ranked_id, &ranked_post), (&unranked_id, &unranked_post)] {
        let uri = post_uri_builder(author_id.clone(), post_id.clone());
        let tag = PubkyAppTag {
            uri: uri.clone(),
            label: label.to_string(),
            created_at: Utc::now().timestamp_millis(),
        };
        test.put(&fan_kp, &tag.hs_path(), tag).await?;
        let reply = PubkyAppPost {
            parent: Some(uri),
            ..short_post("Watcher:Gate:Reply")
        };
        test.create_post(&fan_kp, &reply).await?;
    }

    let ranked_key: &[&str] = &[&ranked_id, &ranked_post];
    assert!(
        check_member_global_timeline_user_post(&ranked_id, &ranked_post)
            .await?
            .is_some()
    );
    assert_eq!(
        check_member_total_engagement_user_posts(ranked_key).await?,
        Some(2),
        "one tag and one reply"
    );
    assert!(check_member_post_tag_global_timeline(ranked_key, label)
        .await?
        .is_some());
    assert_eq!(
        check_member_total_engagement_post_tag(ranked_key, label).await?,
        Some(1)
    );

    let unranked_key: &[&str] = &[&unranked_id, &unranked_post];
    assert!(
        check_member_global_timeline_user_post(&unranked_id, &unranked_post)
            .await?
            .is_none(),
        "global timeline"
    );
    assert!(
        check_member_total_engagement_user_posts(unranked_key)
            .await?
            .is_none(),
        "global engagement"
    );
    assert!(
        check_member_post_tag_global_timeline(unranked_key, label)
            .await?
            .is_none(),
        "tag timeline"
    );
    assert!(
        check_member_total_engagement_post_tag(unranked_key, label)
            .await?
            .is_none(),
        "tag engagement"
    );
    // Their own stream still has it.
    assert!(
        check_member_user_post_timeline(&unranked_id, &unranked_post)
            .await?
            .is_some()
    );
    Ok(())
}
