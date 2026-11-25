use super::utils::find_post_bookmark;
use crate::event_processor::utils::watcher::WatcherTest;
use crate::event_processor::{
    users::utils::find_user_counts, utils::watcher::HomeserverHashIdPath,
};
use anyhow::Result;
use nexus_common::models::event::EventLine;
use nexus_common::models::post::{Bookmark, PostStream};
use pubky::Keypair;
use pubky_social_specs::legacy_v0::{
    post_uri_builder, traits::HashId, PubkyAppBookmark, PubkyAppPost, PubkyAppUser,
};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_bookmark() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    // Step 1: Create a user
    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_homeserver_bookmark".to_string()),
        image: None,
        links: None,
        name: "Watcher:Bookmark:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;
    // Uncapped: the shared event log is append-only and unbounded.
    let (_, events_in_redis_before) = EventLine::get_from_index(None, usize::MAX).await.unwrap();

    // Step 2: Create a post under that user
    let post = PubkyAppPost {
        content: "Watcher:Bookmark:User:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
        lock: None,
    };
    let (post_id, post_path) = test.create_post(&user_kp, &post).await?;

    // Step 3: Add a bookmark to the post. Before create a new user
    let bookmark = PubkyAppBookmark {
        uri: post_uri_builder(user_id.clone(), post_id.clone()),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let bookmark_id = bookmark.create_id();
    let bookmark_path = bookmark.hs_path();

    // Put bookmark
    test.put(&user_kp, &bookmark_path, bookmark).await.unwrap();

    // Step 4: Verify the bookmark exists in Nexus
    // GRAPH_OP: Assert if the event writes the graph
    let user_bookmark = find_post_bookmark(&user_id, &post_id, &user_id)
        .await
        .unwrap();
    assert_eq!(user_bookmark.id, bookmark_id);

    // Verify bookmark counts have increased for this user
    let user_counts = find_user_counts(&user_id).await;
    assert_eq!(user_counts.bookmarks, 1);

    // INDEX_OP: Assert if the event writes the indexes
    let (_, events_in_redis_after) = EventLine::get_from_index(None, usize::MAX).await.unwrap();
    assert!(events_in_redis_after > events_in_redis_before);

    let result_bookmarks = PostStream::get_bookmarked_posts(
        &user_id,
        nexus_common::db::kv::SortOrder::Descending,
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();

    assert_eq!(result_bookmarks.post_keys.len(), 1);
    assert_eq!(
        result_bookmarks.post_keys[0],
        format!("{user_id}:{post_id}")
    );
    assert!(result_bookmarks.last_post_score.is_some());

    let exist_bookmark = Bookmark::get_from_index(&user_id, &post_id, &user_id)
        .await
        .unwrap();
    assert!(exist_bookmark.is_some(), "The bookmark has to be indexed");

    let bookmark = exist_bookmark.unwrap();
    assert_eq!(bookmark.id, bookmark_id, "Bookmark ids does not match");

    // Cleanup user and post
    test.cleanup_post(&user_kp, &post_path).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}

/// A bookmark PUT applied again, as a homeserver re-index or a move to another homeserver does,
/// keeps the time nexus first indexed it, in the graph and in the index: a new one would move
/// the post to the top of the user's bookmarks (as #663 fixed for posts and tags).
#[tokio_shared_rt::test(shared)]
async fn test_bookmark_put_again_keeps_indexed_at_and_order() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_bookmark_put_again_keeps_indexed_at_and_order".to_string()),
        image: None,
        links: None,
        name: "Watcher:BookmarkPutAgain:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    let post = PubkyAppPost {
        content: "Watcher:BookmarkPutAgain:User:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
        lock: None,
    };
    let (first_post_id, first_post_path) = test.create_post(&user_kp, &post).await?;
    let (second_post_id, second_post_path) = test.create_post(&user_kp, &post).await?;

    // Bookmark the first post, then the second
    let bookmark_of = |post_id: &str| PubkyAppBookmark {
        uri: post_uri_builder(user_id.clone(), post_id.to_string()),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let first_bookmark_path = bookmark_of(&first_post_id).hs_path();
    test.put(&user_kp, &first_bookmark_path, bookmark_of(&first_post_id))
        .await?;
    let second_bookmark_path = bookmark_of(&second_post_id).hs_path();
    test.put(
        &user_kp,
        &second_bookmark_path,
        bookmark_of(&second_post_id),
    )
    .await?;

    let first_indexed_at = find_post_bookmark(&user_id, &first_post_id, &user_id)
        .await?
        .indexed_at;

    // The first bookmark's PUT again, through the homeserver. Its index entries are removed
    // first, as if the first attempt had failed before writing them: the repeat must write
    // them back.
    Bookmark::del_from_index(&user_id, &first_post_id, &user_id).await?;
    test.put(&user_kp, &first_bookmark_path, bookmark_of(&first_post_id))
        .await?;

    // GRAPH_OP: the edge keeps its time
    let bookmark_in_graph = find_post_bookmark(&user_id, &first_post_id, &user_id).await?;
    assert_eq!(
        bookmark_in_graph.indexed_at, first_indexed_at,
        "A bookmark written again keeps the time it was first indexed"
    );

    // INDEX_OP: the index is written back with that time, so the user's bookmarks keep their
    // order
    let bookmark_in_index = Bookmark::get_from_index(&user_id, &first_post_id, &user_id)
        .await?
        .expect("The bookmark has to be indexed");
    assert_eq!(bookmark_in_index.indexed_at, first_indexed_at);

    let bookmarks = PostStream::get_bookmarked_posts(
        &user_id,
        nexus_common::db::kv::SortOrder::Descending,
        None,
        None,
        None,
        None,
    )
    .await?;
    assert_eq!(
        bookmarks.post_keys,
        vec![
            format!("{user_id}:{second_post_id}"),
            format!("{user_id}:{first_post_id}")
        ],
        "The bookmark written again stays behind the newer one"
    );

    // Cleanup
    test.del(&user_kp, &first_bookmark_path).await?;
    test.del(&user_kp, &second_bookmark_path).await?;
    test.cleanup_post(&user_kp, &first_post_path).await?;
    test.cleanup_post(&user_kp, &second_post_path).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}
