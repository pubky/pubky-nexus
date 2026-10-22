use super::utils::bookmarked_post_keys;
use crate::event_processor::posts::utils::{collection_post, short_post};
use crate::event_processor::users::utils::find_user_counts;
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use nexus_common::models::post::Bookmark;
use pubky::Keypair;
use pubky_app_specs::{post_uri_builder, PubkyAppBookmark, PubkyAppUser};

/// Bookmarking a collection follows it: it is listed with the user's bookmarks
/// but left out of their bookmark count.
#[tokio_shared_rt::test(shared)]
async fn test_collection_bookmark_is_listed_but_not_counted() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_collection_bookmark_is_listed_but_not_counted".to_string()),
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

    let bookmark_of = |post_id: &str| PubkyAppBookmark {
        uri: post_uri_builder(user_id.clone(), post_id.to_string()),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let short_bookmark = bookmark_of(&short_id);
    let col_bookmark = bookmark_of(&col_id);
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
    assert_eq!(
        find_user_counts(&user_id).await.bookmarks,
        1,
        "only the real post bookmark counts"
    );

    // Unfollowing the collection drops it from the stream and the point read
    test.del(&user_kp, &col_bookmark.hs_path()).await?;
    assert_eq!(
        bookmarked_post_keys(&user_id).await?,
        vec![format!("{user_id}:{short_id}")]
    );
    assert!(Bookmark::get_by_id(&user_id, &col_id, Some(&user_id))
        .await?
        .is_none());
    assert_eq!(find_user_counts(&user_id).await.bookmarks, 1);

    test.cleanup_post(&user_kp, &short_path).await?;
    test.cleanup_post(&user_kp, &col_path).await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}
