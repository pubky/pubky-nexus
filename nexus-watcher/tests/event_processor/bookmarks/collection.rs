use super::utils::{bookmark_of, bookmarked_post_keys};
use crate::event_processor::posts::utils::{collection_post, short_post};
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use nexus_common::models::post::Bookmark;
use pubky::Keypair;
use pubky_app_specs::PubkyAppUser;

/// Bookmarking a collection follows it: it is listed with the user's bookmarks
/// until it is unbookmarked. `test_bookmarking_a_collection_is_excluded_from_bookmarks`
/// pins that it is left out of their bookmark count.
#[tokio_shared_rt::test(shared)]
async fn test_collection_bookmark_is_listed_until_unbookmarked() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_collection_bookmark_is_listed_until_unbookmarked".to_string()),
        image: None,
        links: None,
        name: "Watcher:Bookmark:Collection".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    let (short_id, short_path) = test.create_post(&user_kp, &short_post("plain")).await?;
    let (col_id, col_path) = test
        .create_post(&user_kp, &collection_post("Followed"))
        .await?;

    let short_bookmark = bookmark_of(&user_id, &short_id);
    let col_bookmark = bookmark_of(&user_id, &col_id);
    test.put(&user_kp, &short_bookmark.hs_path(), &short_bookmark)
        .await?;
    test.put(&user_kp, &col_bookmark.hs_path(), &col_bookmark)
        .await?;

    let mut keys = bookmarked_post_keys(&user_id).await?;
    keys.sort();
    let mut expected = vec![
        format!("{user_id}:{short_id}"),
        format!("{user_id}:{col_id}"),
    ];
    expected.sort();
    assert_eq!(
        keys, expected,
        "the collection is listed with the bookmarks"
    );
    assert!(Bookmark::get_by_id(&user_id, &col_id, Some(&user_id))
        .await?
        .is_some());

    // Unfollowing the collection drops it from the stream and the point read
    test.del(&user_kp, &col_bookmark.hs_path()).await?;
    assert_eq!(
        bookmarked_post_keys(&user_id).await?,
        vec![format!("{user_id}:{short_id}")]
    );
    assert!(Bookmark::get_by_id(&user_id, &col_id, Some(&user_id))
        .await?
        .is_none());

    test.cleanup_post(&user_kp, &short_path).await?;
    test.cleanup_post(&user_kp, &col_path).await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}
