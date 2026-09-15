//! COLLECTED edges: a Collection's items are materialized as graph edges on
//! every put, edit and delete, and each touched item's `collections` count is
//! invalidated so the read-through recomputes it. Counts are read before each
//! mutation so the cache is warm and a missed invalidation would show.

use super::utils::{
    collection_post_with_items, find_collections_of, find_post_counts, short_post, test_user,
};
use crate::event_processor::utils::watcher::{generate_post_id, HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use nexus_common::models::post::PostDetails;
use pubky::{Keypair, ResourcePath};
use pubky_app_specs::{
    post_uri_builder, traits::HasIdPath, PubkyAppBookmark, PubkyAppPost, PubkyAppPostKind,
};

/// One user with two short posts to curate.
struct Fixture {
    test: WatcherTest,
    kp: Keypair,
    user_id: String,
    item_a: String,
    item_b: String,
}

async fn fixture(name: &str) -> Result<Fixture> {
    let mut test = WatcherTest::setup(None).await?;
    let kp = Keypair::random();
    let user_id = test
        .create_user(&kp, &test_user(name, "collected edges"))
        .await?;
    let (item_a, _) = test.create_post(&kp, &short_post("item a")).await?;
    let (item_b, _) = test.create_post(&kp, &short_post("item b")).await?;
    Ok(Fixture {
        test,
        kp,
        user_id,
        item_a,
        item_b,
    })
}

fn uri(f: &Fixture, post_id: &str) -> String {
    post_uri_builder(f.user_id.clone(), post_id.to_string())
}

async fn collections_count(f: &Fixture, post_id: &str) -> u32 {
    find_post_counts(&f.user_id, post_id).await.collections
}

async fn curators(f: &Fixture, post_id: &str) -> Vec<(String, String)> {
    find_collections_of(&f.user_id, post_id).await
}

#[tokio_shared_rt::test(shared)]
async fn test_collection_put_links_items_and_counts_them() -> Result<()> {
    let mut f = fixture("Watcher:Collected:Put").await?;
    assert_eq!(collections_count(&f, &f.item_a).await, 0);
    assert_eq!(collections_count(&f, &f.item_b).await, 0);

    let items = [uri(&f, &f.item_a), uri(&f, &f.item_b)];
    let (col_id, _) = f
        .test
        .create_post(&f.kp, &collection_post_with_items("Curated", &items))
        .await?;

    for item in [&f.item_a, &f.item_b] {
        assert_eq!(collections_count(&f, item).await, 1);
        assert_eq!(
            curators(&f, item).await,
            vec![(f.user_id.clone(), col_id.clone())]
        );
    }

    f.test.cleanup_user(&f.kp).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_collection_edit_reconciles_items() -> Result<()> {
    let mut f = fixture("Watcher:Collected:Edit").await?;
    let (item_c, _) = f.test.create_post(&f.kp, &short_post("item c")).await?;
    let (col_id, col_path) = f
        .test
        .create_post(
            &f.kp,
            &collection_post_with_items("Curated", &[uri(&f, &f.item_a), uri(&f, &f.item_b)]),
        )
        .await?;
    assert_eq!(collections_count(&f, &f.item_a).await, 1);
    assert_eq!(collections_count(&f, &f.item_b).await, 1);
    assert_eq!(collections_count(&f, &item_c).await, 0);

    // Identical re-PUT: nothing moves.
    f.test
        .put(
            &f.kp,
            &col_path,
            &collection_post_with_items("Curated", &[uri(&f, &f.item_a), uri(&f, &f.item_b)]),
        )
        .await?;
    assert_eq!(collections_count(&f, &f.item_a).await, 1);
    assert_eq!(collections_count(&f, &f.item_b).await, 1);

    // Drop A, keep B, add C.
    f.test
        .put(
            &f.kp,
            &col_path,
            &collection_post_with_items("Curated", &[uri(&f, &f.item_b), uri(&f, &item_c)]),
        )
        .await?;
    assert_eq!(collections_count(&f, &f.item_a).await, 0, "removed item");
    assert_eq!(collections_count(&f, &f.item_b).await, 1, "kept item");
    assert_eq!(collections_count(&f, &item_c).await, 1, "added item");
    assert!(curators(&f, &f.item_a).await.is_empty());
    assert_eq!(
        curators(&f, &item_c).await,
        vec![(f.user_id.clone(), col_id.clone())]
    );

    // Kind flip Collection -> Short tears the edges down.
    f.test
        .put(&f.kp, &col_path, &short_post("not curated anymore"))
        .await?;
    assert_eq!(collections_count(&f, &f.item_b).await, 0);
    assert_eq!(collections_count(&f, &item_c).await, 0);
    assert!(curators(&f, &f.item_b).await.is_empty());

    f.test.cleanup_user(&f.kp).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_collection_hard_delete_uncounts_items() -> Result<()> {
    let mut f = fixture("Watcher:Collected:HardDel").await?;
    let (_, col_path) = f
        .test
        .create_post(
            &f.kp,
            &collection_post_with_items("Curated", &[uri(&f, &f.item_a), uri(&f, &f.item_b)]),
        )
        .await?;
    assert_eq!(collections_count(&f, &f.item_a).await, 1);
    assert_eq!(collections_count(&f, &f.item_b).await, 1);

    f.test.cleanup_post(&f.kp, &col_path).await?;

    assert_eq!(collections_count(&f, &f.item_a).await, 0);
    assert_eq!(collections_count(&f, &f.item_b).await, 0);
    assert!(curators(&f, &f.item_a).await.is_empty());

    f.test.cleanup_user(&f.kp).await?;
    Ok(())
}

/// A bookmarked collection soft-deletes into a `[DELETED]` Short placeholder,
/// which goes through the edit path and must drop the edges the same way.
#[tokio_shared_rt::test(shared)]
async fn test_collection_soft_delete_uncounts_items() -> Result<()> {
    let mut f = fixture("Watcher:Collected:SoftDel").await?;
    let (col_id, col_path) = f
        .test
        .create_post(
            &f.kp,
            &collection_post_with_items("Curated", &[uri(&f, &f.item_a)]),
        )
        .await?;
    assert_eq!(collections_count(&f, &f.item_a).await, 1);

    let follower_kp = Keypair::random();
    f.test
        .create_user(&follower_kp, &test_user("Watcher:Collected:Follower", ""))
        .await?;
    let bookmark = PubkyAppBookmark {
        uri: uri(&f, &col_id),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    f.test
        .put(&follower_kp, &bookmark.hs_path(), bookmark)
        .await?;

    f.test.cleanup_post(&f.kp, &col_path).await?;

    let placeholder = PostDetails::get_by_id(&f.user_id, &col_id)
        .await?
        .expect("soft-deleted placeholder still present");
    assert_eq!(placeholder.kind, PubkyAppPostKind::Short);
    assert_eq!(placeholder.content, "[DELETED]");
    assert_eq!(collections_count(&f, &f.item_a).await, 0);
    assert!(curators(&f, &f.item_a).await.is_empty());

    f.test.cleanup_user(&f.kp).await?;
    f.test.cleanup_user(&follower_kp).await?;
    Ok(())
}

/// Being curated does not pin a post: it hard-deletes and leaves the collection.
#[tokio_shared_rt::test(shared)]
async fn test_curated_item_still_hard_deletes() -> Result<()> {
    let mut f = fixture("Watcher:Collected:ItemDel").await?;
    let (col_id, _) = f
        .test
        .create_post(
            &f.kp,
            &collection_post_with_items("Curated", &[uri(&f, &f.item_a), uri(&f, &f.item_b)]),
        )
        .await?;

    let item_a_path: ResourcePath = PubkyAppPost::create_path(&f.item_a).parse()?;
    f.test.cleanup_post(&f.kp, &item_a_path).await?;

    assert!(
        PostDetails::get_by_id(&f.user_id, &f.item_a)
            .await?
            .is_none(),
        "a curated item hard-deletes instead of leaving a placeholder"
    );
    assert_eq!(collections_count(&f, &f.item_b).await, 1);
    assert_eq!(
        curators(&f, &f.item_b).await,
        vec![(f.user_id.clone(), col_id)]
    );

    f.test.cleanup_user(&f.kp).await?;
    Ok(())
}

/// An item nexus has not indexed gets no edge; the collection is still indexed
/// and the live items are counted.
#[tokio_shared_rt::test(shared)]
async fn test_collection_skips_unindexed_item() -> Result<()> {
    let mut f = fixture("Watcher:Collected:Unindexed").await?;
    let (col_id, _) = f
        .test
        .create_post(
            &f.kp,
            &collection_post_with_items("Curated", &[uri(&f, &f.item_a), uri(&f, "ZZZZZZZZZZZZZ")]),
        )
        .await?;

    assert_eq!(collections_count(&f, &f.item_a).await, 1);
    assert_eq!(
        curators(&f, &f.item_a).await,
        vec![(f.user_id.clone(), col_id.clone())]
    );
    find_post_counts(&f.user_id, &col_id).await;

    f.test.cleanup_user(&f.kp).await?;
    Ok(())
}

/// Links are materialized when the collection is written: an item that arrives
/// later is not linked until the collection is edited again.
#[tokio_shared_rt::test(shared)]
async fn test_item_indexed_after_collection_links_on_next_edit() -> Result<()> {
    let mut f = fixture("Watcher:Collected:Late").await?;
    let late_id = generate_post_id();
    let late_path: ResourcePath = PubkyAppPost::create_path(&late_id).parse()?;
    let (col_id, col_path) = f
        .test
        .create_post(
            &f.kp,
            &collection_post_with_items("Curated", &[uri(&f, &late_id)]),
        )
        .await?;

    f.test.put(&f.kp, &late_path, &short_post("late")).await?;
    assert_eq!(collections_count(&f, &late_id).await, 0);
    assert!(curators(&f, &late_id).await.is_empty());

    f.test
        .put(
            &f.kp,
            &col_path,
            &collection_post_with_items("Curated", &[uri(&f, &late_id)]),
        )
        .await?;
    assert_eq!(collections_count(&f, &late_id).await, 1);
    assert_eq!(
        curators(&f, &late_id).await,
        vec![(f.user_id.clone(), col_id)]
    );

    f.test.cleanup_user(&f.kp).await?;
    Ok(())
}

/// The spec validates URI shape only, so a collection may list itself.
#[tokio_shared_rt::test(shared)]
async fn test_collection_never_links_itself() -> Result<()> {
    let mut f = fixture("Watcher:Collected:Self").await?;
    let (col_id, col_path) = f
        .test
        .create_post(
            &f.kp,
            &collection_post_with_items("Curated", &[uri(&f, &f.item_a)]),
        )
        .await?;
    f.test
        .put(
            &f.kp,
            &col_path,
            &collection_post_with_items("Curated", &[uri(&f, &f.item_a), uri(&f, &col_id)]),
        )
        .await?;

    assert!(curators(&f, &col_id).await.is_empty(), "no self edge");
    assert_eq!(collections_count(&f, &f.item_a).await, 1);

    f.test.cleanup_user(&f.kp).await?;
    Ok(())
}
