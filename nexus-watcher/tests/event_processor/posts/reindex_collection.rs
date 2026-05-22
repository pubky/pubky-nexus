//! Regression: `PostsByTagSearch::reindex()` rebuilds the by-tag sorted
//! sets from Cypher (`global_tags_by_post` and
//! `global_tags_by_post_engagement`). The watcher write path correctly
//! skips Collections when applying tag-touched index writes, but the
//! reindex Cypher had no kind filter — so after a manual reindex (e.g.
//! Redis flush + rebuild) a tagged Collection would silently surface in
//! `?tags=…` streams, contradicting the watcher's gate.
//!
//! Fix: `WHERE post.kind <> 'collection'` in both queries. This test
//! exercises the reindex path end-to-end with a real tagged Collection
//! in the graph and asserts both sorted sets stay free of the leak.

use super::utils::{collection_post, test_user};
use crate::event_processor::tags::utils::{
    check_member_post_tag_global_timeline, check_member_total_engagement_post_tag,
};
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::post::search::PostsByTagSearch;
use pubky::Keypair;
use pubky_app_specs::{post_uri_builder, PubkyAppTag};

#[tokio_shared_rt::test(shared)]
async fn test_reindex_excludes_tagged_collection_from_by_tag_streams() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let author_kp = Keypair::random();
    let author = test_user("CollAuthor", "test_reindex_collection_exclusion");
    let author_id = test.create_user(&author_kp, &author).await?;

    let collection = collection_post("AI papers", None, vec![]);
    let (coll_id, coll_path) = test.create_post(&author_kp, &collection).await?;

    let tagger_kp = Keypair::random();
    let tagger = test_user("Tagger", "test_reindex_collection_tagger");
    test.create_user(&tagger_kp, &tagger).await?;

    let label = "reindex-collection-label";
    let tag = PubkyAppTag {
        uri: post_uri_builder(author_id.clone(), coll_id.clone()),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();
    test.put(&tagger_kp, &tag_path, tag).await?;

    let coll_key: &[&str] = &[&author_id, &coll_id];

    // Watcher write path: by-tag sorted sets already exclude the Collection
    // (gate at tag.rs). This is the baseline; without it the reindex test
    // below would be vacuous.
    assert!(
        check_member_post_tag_global_timeline(coll_key, label)
            .await?
            .is_none(),
        "pre-condition: tagged Collection must not be in TAG_GLOBAL_POST_TIMELINE after watcher write"
    );
    assert!(
        check_member_total_engagement_post_tag(coll_key, label)
            .await?
            .is_none(),
        "pre-condition: tagged Collection must not be in TAG_GLOBAL_POST_ENGAGEMENT after watcher write"
    );

    // The load-bearing call: reindex rebuilds both sorted sets from Cypher.
    // Pre-fix, the queries lacked `WHERE post.kind <> 'collection'`, so the
    // tagged Collection in the graph would resurface in Redis.
    PostsByTagSearch::reindex().await?;

    assert!(
        check_member_post_tag_global_timeline(coll_key, label)
            .await?
            .is_none(),
        "reindex leaked tagged Collection into TAG_GLOBAL_POST_TIMELINE — global_tags_by_post needs `WHERE post.kind <> 'collection'`"
    );
    assert!(
        check_member_total_engagement_post_tag(coll_key, label)
            .await?
            .is_none(),
        "reindex leaked tagged Collection into TAG_GLOBAL_POST_ENGAGEMENT — global_tags_by_post_engagement needs `WHERE post.kind <> 'collection'`"
    );

    test.del(&tagger_kp, &tag_path).await?;
    test.cleanup_post(&author_kp, &coll_path).await?;
    test.cleanup_user(&author_kp).await?;
    test.cleanup_user(&tagger_kp).await?;
    Ok(())
}
