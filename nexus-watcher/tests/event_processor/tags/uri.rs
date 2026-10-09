use super::utils::find_tag_uri;
use crate::event_processor::posts::utils::{short_post, test_user};
use crate::event_processor::utils::watcher::{
    retrieve_and_handle_event_line, HomeserverHashIdPath, WatcherTest,
};
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::notification::{Notification, NotificationBody, PostChangedSource};
use nexus_common::types::Pagination;
use pubky::{Keypair, ResourcePath};
use pubky_app_specs::traits::HashId;
use pubky_app_specs::{post_uri_builder, tag_uri_builder, user_uri_builder, PubkyAppTag};

fn tag_on(target_uri: String, label: &str) -> PubkyAppTag {
    PubkyAppTag {
        uri: target_uri,
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    }
}

fn mapky_path(tag_id: &str) -> ResourcePath {
    format!("/pub/mapky/tags/{tag_id}").parse().unwrap()
}

/// A user and a post by that user, for the tests below.
async fn author_with_post(
    test: &mut WatcherTest,
    name: &str,
) -> Result<(Keypair, String, String, ResourcePath)> {
    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user(format!("Watcher:TagUri:{name}:Author"), "uri"),
        )
        .await?;
    let (post_id, post_path) = test
        .create_post(
            &author_kp,
            &short_post(format!("Watcher:TagUri:{name}:Post")),
        )
        .await?;
    Ok((author_kp, author_id, post_id, post_path))
}

async fn new_user(test: &mut WatcherTest, name: &str) -> Result<(Keypair, String)> {
    let user_kp = Keypair::random();
    let user_id = test
        .create_user(
            &user_kp,
            &test_user(format!("Watcher:TagUri:{name}"), "uri"),
        )
        .await?;
    Ok((user_kp, user_id))
}

/// A `pubky.app` tag on a post stores its event path as the edge's `uri`.
#[tokio_shared_rt::test(shared)]
async fn test_pubky_app_post_tag_stores_event_path_as_uri() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (author_kp, author_id, post_id, post_path) = author_with_post(&mut test, "PostTag").await?;
    let (tagger_kp, tagger_id) = new_user(&mut test, "PostTag:Tagger").await?;

    let tag = tag_on(post_uri_builder(author_id, post_id), "uri-pubky-app-post");
    let tag_id = tag.create_id();
    let tag_path = tag.hs_path();
    test.put(&tagger_kp, &tag_path, tag).await?;

    assert_eq!(
        find_tag_uri(&tagger_id, &tag_id).await,
        Some(tag_uri_builder(tagger_id.clone(), tag_id.clone()))
    );

    test.del(&tagger_kp, &tag_path).await?;
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&tagger_kp).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}

/// A `pubky.app` tag on a user stores its event path as the edge's `uri`.
#[tokio_shared_rt::test(shared)]
async fn test_pubky_app_user_tag_stores_event_path_as_uri() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (tagged_kp, tagged_id) = new_user(&mut test, "UserTag:Tagged").await?;
    let (tagger_kp, tagger_id) = new_user(&mut test, "UserTag:Tagger").await?;

    let tag = tag_on(user_uri_builder(tagged_id), "uri-pubky-app-user");
    let tag_id = tag.create_id();
    let tag_path = tag.hs_path();
    test.put(&tagger_kp, &tag_path, tag).await?;

    assert_eq!(
        find_tag_uri(&tagger_id, &tag_id).await,
        Some(tag_uri_builder(tagger_id.clone(), tag_id.clone()))
    );

    test.del(&tagger_kp, &tag_path).await?;
    test.cleanup_user(&tagger_kp).await?;
    test.cleanup_user(&tagged_kp).await?;
    Ok(())
}

/// A tag in another app's folder that targets a post stores that folder's address.
#[tokio_shared_rt::test(shared)]
async fn test_mapky_post_tag_stores_mapky_uri() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (author_kp, author_id, post_id, post_path) = author_with_post(&mut test, "Mapky").await?;
    let (tagger_kp, tagger_id) = new_user(&mut test, "Mapky:Tagger").await?;

    let tag = tag_on(post_uri_builder(author_id, post_id), "uri-mapky-post");
    let tag_id = tag.create_id();
    test.put(&tagger_kp, &mapky_path(&tag_id), tag).await?;

    assert_eq!(
        find_tag_uri(&tagger_id, &tag_id).await,
        Some(format!("pubky://{tagger_id}/pub/mapky/tags/{tag_id}"))
    );

    test.del(&tagger_kp, &mapky_path(&tag_id)).await?;
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&tagger_kp).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}

/// A tag in another app's folder that targets a URL stores that folder's address on the `Resource` edge.
#[tokio_shared_rt::test(shared)]
async fn test_mapky_resource_tag_stores_mapky_uri() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (tagger_kp, tagger_id) = new_user(&mut test, "Resource:Tagger").await?;

    let tag = tag_on(
        format!("https://example.com/tag-uri/{tagger_id}"),
        "uri-mapky-resource",
    );
    let tag_id = tag.create_id();
    test.put(&tagger_kp, &mapky_path(&tag_id), tag).await?;

    assert_eq!(
        find_tag_uri(&tagger_id, &tag_id).await,
        Some(format!("pubky://{tagger_id}/pub/mapky/tags/{tag_id}"))
    );

    test.del(&tagger_kp, &mapky_path(&tag_id)).await?;
    test.cleanup_user(&tagger_kp).await?;
    Ok(())
}

/// A tag handled from a stored event line, as the retry processor does, stores the same `uri`.
#[tokio_shared_rt::test(shared)]
async fn test_replayed_tag_event_line_stores_uri() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (author_kp, author_id, post_id, post_path) = author_with_post(&mut test, "Replay").await?;
    let (tagger_kp, tagger_id) = new_user(&mut test, "Replay:Tagger").await?;

    // Leave the tag event to be handled from its line below
    test = test.remove_event_processing().await;
    let tag = tag_on(post_uri_builder(author_id, post_id), "uri-replay");
    let tag_id = tag.create_id();
    test.put(&tagger_kp, &mapky_path(&tag_id), tag).await?;

    let expected = format!("pubky://{tagger_id}/pub/mapky/tags/{tag_id}");
    let event_handler = test.event_processor_runner.event_handler.clone();
    let line = format!("PUT {expected}");
    retrieve_and_handle_event_line(&line, event_handler.clone()).await?;
    assert_eq!(
        find_tag_uri(&tagger_id, &tag_id).await,
        Some(expected.clone())
    );

    // A retry of the same line lands on the existing edge and leaves `uri` unchanged
    retrieve_and_handle_event_line(&line, event_handler).await?;
    assert_eq!(find_tag_uri(&tagger_id, &tag_id).await, Some(expected));

    test.del(&tagger_kp, &mapky_path(&tag_id)).await?;
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&tagger_kp).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}

/// When a tagged post is edited, the tagger is told the address of their own tag file.
#[tokio_shared_rt::test(shared)]
async fn test_tagged_post_edit_notification_links_tag_uri() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (author_kp, author_id, post_id, post_path) = author_with_post(&mut test, "Notify").await?;
    let (tagger_kp, tagger_id) = new_user(&mut test, "Notify:Tagger").await?;

    let tag = tag_on(post_uri_builder(author_id, post_id), "uri-notify");
    let tag_id = tag.create_id();
    test.put(&tagger_kp, &mapky_path(&tag_id), tag).await?;

    test.put(
        &author_kp,
        &post_path,
        short_post("Watcher:TagUri:Notify:Edited"),
    )
    .await?;

    let edited: Vec<(PostChangedSource, String)> =
        Notification::get_by_id(&tagger_id, Pagination::default())
            .await
            .unwrap()
            .into_iter()
            .filter_map(|notification| match notification.body {
                NotificationBody::PostEdited {
                    edit_source,
                    linked_uri,
                    ..
                } => Some((edit_source, linked_uri)),
                _ => None,
            })
            .collect();
    assert_eq!(
        edited,
        vec![(
            PostChangedSource::TaggedPost,
            format!("pubky://{tagger_id}/pub/mapky/tags/{tag_id}")
        )]
    );

    test.del(&tagger_kp, &mapky_path(&tag_id)).await?;
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&tagger_kp).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}
