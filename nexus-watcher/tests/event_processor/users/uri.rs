use super::utils::{find_user_details, find_user_uri, set_user_uri};
use crate::event_processor::posts::utils::{short_post, test_user};
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::tag::view::TagView;
use pubky::Keypair;
use pubky_app_specs::traits::HashId;
use pubky_app_specs::{user_uri_builder, PubkyAppTag};

fn tag_on_user(user_id: &str, label: &str) -> PubkyAppTag {
    PubkyAppTag {
        uri: user_uri_builder(user_id.to_string()),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    }
}

/// A profile PUT stores the event path as the user's `uri`, and an edit keeps it.
#[tokio_shared_rt::test(shared)]
async fn test_user_put_stores_event_path_as_uri() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(&user_kp, &test_user("Watcher:UserUri:Put", "uri"))
        .await?;

    let expected = user_uri_builder(user_id.clone());
    assert_eq!(find_user_uri(&user_id).await, Some(expected.clone()));

    test.create_profile(&user_kp, &test_user("Watcher:UserUri:Put:Edited", "uri"))
        .await?;
    assert_eq!(
        find_user_details(&user_id).await?.name,
        "Watcher:UserUri:Put:Edited"
    );
    assert_eq!(find_user_uri(&user_id).await, Some(expected));

    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// Deleting a profile that has relationships leaves a tombstone, which keeps the stored `uri`.
#[tokio_shared_rt::test(shared)]
async fn test_user_tombstone_keeps_uri() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(&user_kp, &test_user("Watcher:UserUri:Tombstone", "uri"))
        .await?;
    let (_post_id, post_path) = test
        .create_post(&user_kp, &short_post("Watcher:UserUri:Tombstone:Post"))
        .await?;

    test.cleanup_user(&user_kp).await?;

    assert!(find_user_details(&user_id).await?.deleted);
    assert_eq!(
        find_user_uri(&user_id).await,
        Some(user_uri_builder(user_id.clone()))
    );

    test.cleanup_post(&user_kp, &post_path).await?;
    Ok(())
}

/// A tag view on a user reads the user's stored `uri`.
#[tokio_shared_rt::test(shared)]
async fn test_tag_view_reads_stored_user_uri() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let tagged_kp = Keypair::random();
    let tagged_id = test
        .create_user(&tagged_kp, &test_user("Watcher:UserUri:View:Tagged", "uri"))
        .await?;
    let tagger_kp = Keypair::random();
    let tagger_id = test
        .create_user(&tagger_kp, &test_user("Watcher:UserUri:View:Tagger", "uri"))
        .await?;

    let tag = tag_on_user(&tagged_id, "uri-user-view");
    let tag_id = tag.create_id();
    let tag_path = tag.hs_path();
    test.put(&tagger_kp, &tag_path, tag).await?;

    // An address only the stored value can produce
    let stored = format!("pubky://{tagged_id}/pub/social/v1/profile.json");
    set_user_uri(&tagged_id, Some(&stored)).await?;

    let view = TagView::get_by_tagger_and_id(&tagger_id, &tag_id)
        .await?
        .expect("tag view");
    assert_eq!(view.uri, stored);

    test.del(&tagger_kp, &tag_path).await?;
    test.cleanup_user(&tagger_kp).await?;
    test.cleanup_user(&tagged_kp).await?;
    Ok(())
}

/// A stub user, never read from a profile.json, stores no `uri`.
#[tokio_shared_rt::test(shared)]
async fn test_stub_user_stores_no_uri() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    // Signed up on the homeserver, but no profile.json
    let stub_kp = Keypair::random();
    let stub_id = stub_kp.public_key().to_z32();
    test.register_user(&stub_kp).await?;

    let tagger_kp = Keypair::random();
    test.create_user(&tagger_kp, &test_user("Watcher:UserUri:Stub:Tagger", "uri"))
        .await?;

    // Tagging an unknown user ingests it as a stub
    let tag = tag_on_user(&stub_id, "uri-user-stub");
    let tag_path = tag.hs_path();
    test.put(&tagger_kp, &tag_path, tag).await?;

    assert!(find_user_details(&stub_id).await.is_ok(), "stub node");
    assert_eq!(find_user_uri(&stub_id).await, None);

    test.del(&tagger_kp, &tag_path).await?;
    test.cleanup_user(&tagger_kp).await?;
    Ok(())
}

/// A tag view on a user with no stored `uri` has an empty `uri`.
#[tokio_shared_rt::test(shared)]
async fn test_tag_view_on_user_without_uri_is_empty() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let tagged_kp = Keypair::random();
    let tagged_id = test
        .create_user(
            &tagged_kp,
            &test_user("Watcher:UserUri:Fallback:Tagged", "uri"),
        )
        .await?;
    let tagger_kp = Keypair::random();
    let tagger_id = test
        .create_user(
            &tagger_kp,
            &test_user("Watcher:UserUri:Fallback:Tagger", "uri"),
        )
        .await?;

    let tag = tag_on_user(&tagged_id, "uri-user-fallback");
    let tag_id = tag.create_id();
    let tag_path = tag.hs_path();
    test.put(&tagger_kp, &tag_path, tag).await?;

    // As a stub, or a profile the backfill found no event line for
    set_user_uri(&tagged_id, None).await?;

    let view = TagView::get_by_tagger_and_id(&tagger_id, &tag_id)
        .await?
        .expect("tag view");
    assert_eq!(view.uri, "");

    test.del(&tagger_kp, &tag_path).await?;
    test.cleanup_user(&tagger_kp).await?;
    test.cleanup_user(&tagged_kp).await?;
    Ok(())
}
