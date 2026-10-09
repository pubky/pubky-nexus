//! The ranked timeline sets (`source=all` for viewers inside the trust
//! ranking) mirror the global and per-label timelines as posts and tags are
//! written: only authors in the ranking get in, and deletes come out of both.
//!
//! Each test ranks its own fresh author by adding it to the shared ranking
//! (`Sorted:Users:SocialGraph`) and removes it at the end. No other watcher
//! test rebuilds the ranking, so the member survives for the test's duration.
use super::utils::{check_member_global_timeline_user_post, short_post, test_user};
use crate::event_processor::tags::utils::check_member_post_tag_global_timeline;
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::db::RedisOps;
use nexus_common::models::post::{
    PostStream, POST_RANKED_TIMELINE_KEY_PARTS, TAG_RANKED_POST_TIMELINE,
};
use nexus_common::models::user::USER_SOCIAL_GRAPH_KEY_PARTS;
use pubky::Keypair;
use pubky_app_specs::{post_uri_builder, PubkyAppTag};

const BIO: &str = "ranked timeline mirror";

async fn rank(user_id: &str) -> Result<()> {
    // Far past any fixture rank, so the fixture's positions are untouched.
    PostStream::put_index_sorted_set(&USER_SOCIAL_GRAPH_KEY_PARTS, &[(1e9, user_id)], None, None)
        .await?;
    Ok(())
}

async fn unrank(user_id: &str) -> Result<()> {
    PostStream::remove_from_index_sorted_set(None, &USER_SOCIAL_GRAPH_KEY_PARTS, &[user_id])
        .await?;
    Ok(())
}

/// The post's score in a ranked set, `None` when absent.
async fn ranked_score(key_parts: &[&str], author_id: &str, post_id: &str) -> Result<Option<isize>> {
    Ok(PostStream::check_sorted_set_member(None, key_parts, &[author_id, post_id]).await?)
}

#[tokio_shared_rt::test(shared)]
async fn test_ranked_timeline_mirrors_root_posts() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let ranked_kp = Keypair::random();
    let ranked_id = test
        .create_user(&ranked_kp, &test_user("Watcher:Ranked:Author", BIO))
        .await?;
    let unranked_kp = Keypair::random();
    let unranked_id = test
        .create_user(&unranked_kp, &test_user("Watcher:Unranked:Author", BIO))
        .await?;
    rank(&ranked_id).await?;

    let (ranked_post, ranked_path) = test
        .create_post(&ranked_kp, &short_post("Watcher:Ranked:Post"))
        .await?;
    let (unranked_post, _) = test
        .create_post(&unranked_kp, &short_post("Watcher:Unranked:Post"))
        .await?;

    let global = check_member_global_timeline_user_post(&ranked_id, &ranked_post).await?;
    let ranked = ranked_score(&POST_RANKED_TIMELINE_KEY_PARTS, &ranked_id, &ranked_post).await?;
    assert!(global.is_some(), "the post is in the global timeline");
    assert_eq!(ranked, global, "mirrored at the global score");

    assert!(
        check_member_global_timeline_user_post(&unranked_id, &unranked_post)
            .await?
            .is_some()
    );
    assert!(
        ranked_score(
            &POST_RANKED_TIMELINE_KEY_PARTS,
            &unranked_id,
            &unranked_post
        )
        .await?
        .is_none(),
        "an unranked author's post stays out of the ranked timeline"
    );

    test.cleanup_post(&ranked_kp, &ranked_path).await?;
    assert!(
        ranked_score(&POST_RANKED_TIMELINE_KEY_PARTS, &ranked_id, &ranked_post)
            .await?
            .is_none(),
        "a deleted post leaves the ranked timeline"
    );

    unrank(&ranked_id).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_ranked_tag_timeline_mirrors_tags() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let label = "watcherrankedtag";
    let ranked_set = [&TAG_RANKED_POST_TIMELINE[..], &[label]].concat();

    let ranked_kp = Keypair::random();
    let ranked_id = test
        .create_user(&ranked_kp, &test_user("Watcher:RankedTag:Author", BIO))
        .await?;
    let unranked_kp = Keypair::random();
    let unranked_id = test
        .create_user(&unranked_kp, &test_user("Watcher:UnrankedTag:Author", BIO))
        .await?;
    let tagger_kp = Keypair::random();
    test.create_user(&tagger_kp, &test_user("Watcher:RankedTag:Tagger", BIO))
        .await?;
    rank(&ranked_id).await?;

    let (ranked_post, _) = test
        .create_post(&ranked_kp, &short_post("Watcher:RankedTag:Post"))
        .await?;
    let (unranked_post, _) = test
        .create_post(&unranked_kp, &short_post("Watcher:UnrankedTag:Post"))
        .await?;

    let mut tag_paths = Vec::new();
    for (author_id, post_id) in [(&ranked_id, &ranked_post), (&unranked_id, &unranked_post)] {
        let tag = PubkyAppTag {
            uri: post_uri_builder(author_id.clone(), post_id.clone()),
            label: label.to_string(),
            created_at: Utc::now().timestamp_millis(),
        };
        let tag_path = tag.hs_path();
        test.put(&tagger_kp, &tag_path, tag).await?;
        tag_paths.push(tag_path);
    }

    let source = check_member_post_tag_global_timeline(&[&ranked_id, &ranked_post], label).await?;
    assert!(source.is_some(), "the post is in the label's timeline");
    assert_eq!(
        ranked_score(&ranked_set, &ranked_id, &ranked_post).await?,
        source,
        "mirrored at the label timeline's score"
    );
    assert!(
        check_member_post_tag_global_timeline(&[&unranked_id, &unranked_post], label)
            .await?
            .is_some()
    );
    assert!(
        ranked_score(&ranked_set, &unranked_id, &unranked_post)
            .await?
            .is_none(),
        "an unranked author's post stays out of the label's ranked timeline"
    );

    // The only tagger removes the label: the post leaves both timelines.
    test.del(&tagger_kp, &tag_paths[0]).await?;
    assert!(
        check_member_post_tag_global_timeline(&[&ranked_id, &ranked_post], label)
            .await?
            .is_none()
    );
    assert!(
        ranked_score(&ranked_set, &ranked_id, &ranked_post)
            .await?
            .is_none(),
        "an untagged post leaves the label's ranked timeline"
    );

    unrank(&ranked_id).await?;
    Ok(())
}
