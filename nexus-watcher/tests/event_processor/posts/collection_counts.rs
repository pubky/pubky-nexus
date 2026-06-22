use crate::event_processor::users::utils::find_user_counts;
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use nexus_common::models::post::PostDetails;
use nexus_common::models::user::UserCounts;
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder, PubkyAppBookmark, PubkyAppPost, PubkyAppPostKind, PubkyAppUser,
};

/// Collection posts require a JSON envelope (`name` + `items`) in `content`.
fn collection_post(name: &str) -> PubkyAppPost {
    let content = serde_json::json!({ "name": name, "items": [] }).to_string();
    PubkyAppPost {
        content,
        kind: PubkyAppPostKind::Collection,
        parent: None,
        embed: None,
        attachments: None,
    }
}

fn short_post(content: &str) -> PubkyAppPost {
    PubkyAppPost {
        content: content.to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    }
}

/// Bookmarks `owner_id`'s post (a real bookmark, or "following" a collection).
async fn bookmark_post(
    test: &mut WatcherTest,
    user_kp: &Keypair,
    owner_id: &str,
    post_id: &str,
) -> Result<()> {
    let bookmark = PubkyAppBookmark {
        uri: post_uri_builder(owner_id.to_string(), post_id.to_string()),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let bookmark_path = bookmark.hs_path();
    test.put(user_kp, &bookmark_path, bookmark).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_collection_post_increments_collections_not_replies() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Watcher:CollectionCounts:Put".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    let (_short_id, short_path) = test.create_post(&user_kp, &short_post("plain")).await?;
    let (_col_id, col_path) = test
        .create_post(&user_kp, &collection_post("My Collection"))
        .await?;

    let counts = find_user_counts(&user_id).await;
    assert_eq!(
        counts.posts, 2,
        "posts is the grand total incl. the collection"
    );
    assert_eq!(counts.collections, 1, "the collection is counted once");
    assert_eq!(counts.replies, 0, "a collection is not a reply");

    // Graph backfill must match the live counters.
    let graph_counts = UserCounts::get_from_graph(&user_id)
        .await?
        .expect("user counts present in graph");
    assert_eq!(graph_counts.posts, 2);
    assert_eq!(graph_counts.collections, 1);
    assert_eq!(graph_counts.replies, 0);

    test.cleanup_post(&user_kp, &short_path).await?;
    test.cleanup_post(&user_kp, &col_path).await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_delete_collection_decrements_collections() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Watcher:CollectionCounts:Del".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    let (_col_id, col_path) = test
        .create_post(&user_kp, &collection_post("Doomed"))
        .await?;

    let before = find_user_counts(&user_id).await;
    assert_eq!(before.collections, 1);
    assert_eq!(before.posts, 1);

    test.cleanup_post(&user_kp, &col_path).await?;

    let after = find_user_counts(&user_id).await;
    assert_eq!(after.collections, 0, "collections decremented on delete");
    assert_eq!(after.posts, 0);

    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// Editing a post across the collection boundary moves `collections`; the graph
/// kind is overwritten on every PUT, so otherwise the live counter drifts from
/// the Cypher backfill until the next reindex.
#[tokio_shared_rt::test(shared)]
async fn test_editing_post_kind_moves_collections_counter() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Watcher:CollectionCounts:Edit".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    let (_id, path) = test.create_post(&user_kp, &short_post("draft")).await?;
    assert_eq!(find_user_counts(&user_id).await.collections, 0);

    // Short -> Collection.
    test.put(&user_kp, &path, &collection_post("Now curated"))
        .await?;
    assert_eq!(
        find_user_counts(&user_id).await.collections,
        1,
        "edit into a collection increments collections"
    );

    // Collection -> Short.
    test.put(&user_kp, &path, &short_post("never mind")).await?;
    assert_eq!(
        find_user_counts(&user_id).await.collections,
        0,
        "edit out of a collection decrements collections"
    );

    // Graph backfill agrees with the final live state.
    let graph = UserCounts::get_from_graph(&user_id)
        .await?
        .expect("user counts present in graph");
    assert_eq!(graph.collections, 0);

    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// Deleting a collection that still has relationships soft-deletes it (rewrites
/// it as a `[DELETED]` Short via the edit path). The author's `collections` must
/// drop by exactly one, and the kind transition must persist.
#[tokio_shared_rt::test(shared)]
async fn test_soft_deleting_a_bookmarked_collection_decrements_collections_once() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let author_kp = Keypair::random();
    let author = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Watcher:CollectionCounts:SoftDelAuthor".to_string(),
        status: None,
    };
    let author_id = test.create_user(&author_kp, &author).await?;
    let (col_id, col_path) = test
        .create_post(&author_kp, &collection_post("Curated"))
        .await?;
    assert_eq!(find_user_counts(&author_id).await.collections, 1);

    // A follower bookmarks it, so deleting the collection soft-deletes (keeps the
    // node) instead of hard-deleting.
    let follower_kp = Keypair::random();
    let follower = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Watcher:CollectionCounts:Follower".to_string(),
        status: None,
    };
    let _follower_id = test.create_user(&follower_kp, &follower).await?;
    bookmark_post(&mut test, &follower_kp, &author_id, &col_id).await?;

    test.cleanup_post(&author_kp, &col_path).await?;

    assert_eq!(
        find_user_counts(&author_id).await.collections,
        0,
        "soft-deleting the collection decrements collections exactly once"
    );
    // The post survives as a soft-deleted Short placeholder.
    let details = PostDetails::get_by_id(&author_id, &col_id)
        .await?
        .expect("soft-deleted placeholder still present");
    assert_eq!(details.kind, PubkyAppPostKind::Short);
    assert_eq!(details.content, "[DELETED]");

    test.cleanup_user(&author_kp).await?;
    test.cleanup_user(&follower_kp).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_bookmarking_a_collection_is_excluded_from_bookmarks() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Watcher:CollectionCounts:Bookmark".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    let (short_id, _short_path) = test.create_post(&user_kp, &short_post("plain")).await?;
    let (col_id, _col_path) = test
        .create_post(&user_kp, &collection_post("Followed"))
        .await?;

    bookmark_post(&mut test, &user_kp, &user_id, &short_id).await?; // real bookmark: counts
    bookmark_post(&mut test, &user_kp, &user_id, &col_id).await?; // collection-follow: excluded

    let counts = find_user_counts(&user_id).await;
    assert_eq!(
        counts.bookmarks, 1,
        "only the real post bookmark counts, not the collection-follow"
    );

    let graph_counts = UserCounts::get_from_graph(&user_id)
        .await?
        .expect("user counts present in graph");
    assert_eq!(graph_counts.bookmarks, 1);

    test.cleanup_user(&user_kp).await?;
    Ok(())
}
