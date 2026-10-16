use super::utils::{find_post_details, find_post_uri, set_post_uri, short_post, test_user};
use crate::event_processor::utils::watcher::{retrieve_and_handle_event_line, WatcherTest};
use anyhow::Result;
use nexus_common::models::post::PostDetails;
use nexus_watcher::events::retry::{IndexKey, RetryEvent};
use pubky::{Keypair, ResourcePath};
use pubky_app_specs::traits::HasIdPath;
use pubky_app_specs::{post_uri_builder, PubkyAppPost};

/// A post PUT stores the event path as the post's `uri`.
#[tokio_shared_rt::test(shared)]
async fn test_post_put_stores_event_path_as_uri() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(&user_kp, &test_user("Watcher:PostUri:Put", "test_post_uri"))
        .await?;
    let (post_id, post_path) = test
        .create_post(&user_kp, &short_post("Watcher:PostUri:Put:Post"))
        .await?;

    let expected = post_uri_builder(user_id.clone(), post_id.clone());
    assert_eq!(
        find_post_uri(&user_id, &post_id).await,
        Some(expected.clone())
    );

    let cached = PostDetails::get_from_index(&user_id, &post_id)
        .await?
        .expect("post details in Redis");
    assert_eq!(cached.uri, expected);

    test.cleanup_post(&user_kp, &post_path).await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// A post handled from a stored event line, as the retry processor does, stores the same `uri`.
#[tokio_shared_rt::test(shared)]
async fn test_post_replayed_event_line_stores_uri() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(
            &user_kp,
            &test_user("Watcher:PostUri:Retry", "test_post_uri"),
        )
        .await?;

    // Leave the post event to be handled from its line below
    test = test.remove_event_processing().await;
    let (post_id, post_path) = test
        .create_post(&user_kp, &short_post("Watcher:PostUri:Retry:Post"))
        .await?;

    let expected = post_uri_builder(user_id.clone(), post_id.clone());
    let event_handler = test.event_processor_runner.event_handler.clone();
    retrieve_and_handle_event_line(&format!("PUT {expected}"), event_handler).await?;

    assert_eq!(find_post_uri(&user_id, &post_id).await, Some(expected));

    test.cleanup_post(&user_kp, &post_path).await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// The graph read returns the stored `uri`; no address is built at read time.
#[tokio_shared_rt::test(shared)]
async fn test_post_graph_read_returns_stored_uri() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(
            &user_kp,
            &test_user("Watcher:PostUri:Read", "test_post_uri"),
        )
        .await?;
    let (post_id, post_path) = test
        .create_post(&user_kp, &short_post("Watcher:PostUri:Read:Post"))
        .await?;

    let stored = format!("pubky://{user_id}/pub/social/v1/posts/{post_id}");
    set_post_uri(&user_id, &post_id, Some(&stored)).await?;
    let (details, _) = PostDetails::get_from_graph(&user_id, &post_id)
        .await?
        .expect("post in graph");
    assert_eq!(details.uri, stored);

    test.cleanup_post(&user_kp, &post_path).await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// A file at an alias path (`posts/ID/shadow`) parses to the same post, but its PUT and DEL are
/// skipped: the post keeps its content and canonical `uri`, and is not deleted.
#[tokio_shared_rt::test(shared)]
async fn test_post_alias_path_events_are_skipped() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(
            &user_kp,
            &test_user("Watcher:PostUri:Alias", "test_post_uri"),
        )
        .await?;
    let (post_id, post_path) = test
        .create_post(&user_kp, &short_post("Watcher:PostUri:Alias:Post"))
        .await?;
    let canonical = post_uri_builder(user_id.clone(), post_id.clone());

    // PUT at the alias path, with different content
    let alias_path: ResourcePath =
        format!("{}/shadow", PubkyAppPost::create_path(&post_id)).parse()?;
    test.put(
        &user_kp,
        &alias_path,
        short_post("Watcher:PostUri:Alias:Shadow"),
    )
    .await?;

    let details = find_post_details(&user_id, &post_id).await?;
    assert_eq!(details.content, "Watcher:PostUri:Alias:Post");
    assert_eq!(details.uri, canonical);
    let alias_uri = format!("{canonical}/shadow");
    assert!(!RetryEvent::check_index_key(&IndexKey::for_uri(&alias_uri)).await?);

    // DEL at the alias path
    test.del(&user_kp, &alias_path).await?;

    let details = find_post_details(&user_id, &post_id).await?;
    assert!(!details.deleted, "the alias DEL must not touch the post");
    assert_eq!(details.content, "Watcher:PostUri:Alias:Post");
    assert_eq!(details.uri, canonical);
    assert!(!RetryEvent::check_index_key(&IndexKey::for_uri(&alias_uri)).await?);

    test.cleanup_post(&user_kp, &post_path).await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}
