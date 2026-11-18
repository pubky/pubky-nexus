use super::utils::{
    find_cached_user_details, find_user_details, find_user_epoch, find_user_uri, set_user_epoch,
};
use crate::event_processor::follows::utils::find_follow_relationship;
use crate::event_processor::posts::utils::{
    find_post_details, find_post_uri, short_post, test_user, v1_note, v1_test_user,
};
use crate::event_processor::tags::utils::find_post_tag;
use crate::event_processor::utils::watcher::{
    generate_post_id, retrieve_and_handle_event_line, WatcherTest,
};
use anyhow::Result;
use nexus_common::models::event::EventLine;
use nexus_common::models::user::Epoch;
use nexus_watcher::events::retry::{IndexKey, RetryEvent};
use pubky::{Keypair, ResourcePath};
use pubky_social_specs::legacy_v0::post_uri_builder;
use pubky_social_specs::{post_uri_builder as v1_post_uri_builder, PubkySocialTag};

/// Whether the shared event log holds `line`.
async fn event_line_stored(line: &str) -> Result<bool> {
    // Uncapped: the shared event log is append-only and unbounded.
    let (lines, _) = EventLine::get_from_index(None, usize::MAX).await?;
    Ok(lines.iter().any(|stored| stored == line))
}

/// The event line of `kind` on `user_id`'s `path`.
fn line(kind: &str, user_id: &str, path: &ResourcePath) -> String {
    format!("{kind} {}", uri(user_id, path))
}

/// The address of `user_id`'s `path`.
fn uri(user_id: &str, path: &ResourcePath) -> String {
    format!("pubky://{user_id}{path}")
}

/// Whether the retry queue holds an entry for `uri`: an ignored event leaves none, a failed
/// one may.
async fn queued_for_retry(uri: &str) -> Result<bool> {
    Ok(RetryEvent::check_index_key(&IndexKey::for_uri(uri)).await?)
}

/// A `pubky.app` post PUT of a user at `v1` is ignored: the post is not indexed and the event
/// line is not stored.
#[tokio_shared_rt::test(shared)]
async fn test_v1_user_pubky_app_post_put_is_ignored() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(&user_kp, &test_user("Watcher:EpochGate:Put", "epoch_gate"))
        .await?;
    set_user_epoch(&user_id, "v1").await?;

    let (post_id, post_path) = test
        .create_post(&user_kp, &short_post("Watcher:EpochGate:Put:Post"))
        .await?;

    assert!(
        find_post_details(&user_id, &post_id).await.is_err(),
        "a v1 user's pubky.app post must not be indexed"
    );
    let uri = post_uri_builder(user_id.clone(), post_id);
    assert!(
        !event_line_stored(&format!("PUT {uri}")).await?,
        "an ignored event must store no event line"
    );
    assert!(!queued_for_retry(&uri).await?);

    // Ignored as well: there is no post to delete
    test.cleanup_post(&user_kp, &post_path).await?;
    // Back to `v0`, so the gate lets the cleanup through
    set_user_epoch(&user_id, "v0").await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// A `pubky.app` DEL of a user at `v1` is ignored: a post indexed before stays in place.
#[tokio_shared_rt::test(shared)]
async fn test_v1_user_pubky_app_post_del_is_ignored() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(&user_kp, &test_user("Watcher:EpochGate:Del", "epoch_gate"))
        .await?;
    let (post_id, post_path) = test
        .create_post(&user_kp, &short_post("Watcher:EpochGate:Del:Post"))
        .await?;
    set_user_epoch(&user_id, "v1").await?;

    test.cleanup_post(&user_kp, &post_path).await?;

    let post = find_post_details(&user_id, &post_id).await?;
    assert_eq!(post.content, "Watcher:EpochGate:Del:Post");
    let uri = post_uri_builder(user_id.clone(), post_id);
    let del_line = format!("DEL {uri}");
    assert!(
        !event_line_stored(&del_line).await?,
        "an ignored event must store no event line"
    );
    assert!(!queued_for_retry(&uri).await?);

    // Back to `v0`, then delete the post the homeserver no longer holds
    set_user_epoch(&user_id, "v0").await?;
    let event_handler = test.event_processor_runner.event_handler.clone();
    retrieve_and_handle_event_line(&del_line, event_handler).await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// A `social/v1` post PUT of a user at `v0` is ignored: a v1 copy of the user's `pubky.app`
/// post leaves it as it was, a post only in `social/v1` is not indexed, and neither event line
/// is stored.
#[tokio_shared_rt::test(shared)]
async fn test_v0_user_social_v1_post_put_is_ignored() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(
            &user_kp,
            &test_user("Watcher:EpochGate:V1Put", "epoch_gate"),
        )
        .await?;
    let (post_id, post_path) = test
        .create_post(&user_kp, &short_post("Watcher:EpochGate:V1Put:Post"))
        .await?;
    let before = find_post_details(&user_id, &post_id).await?;
    let uri_before = find_post_uri(&user_id, &post_id).await;

    // The migrator's copy of the post, then a post written to `social/v1` only
    let copy_path = test
        .put_v1_post_version(
            &user_kp,
            &post_id,
            &post_id,
            &v1_note("Watcher:EpochGate:V1Put:Copy"),
        )
        .await?;
    let new_id = generate_post_id();
    let new_path = test
        .put_v1_post_version(
            &user_kp,
            &new_id,
            &new_id,
            &v1_note("Watcher:EpochGate:V1Put:New"),
        )
        .await?;

    let after = find_post_details(&user_id, &post_id).await?;
    assert_eq!(after.content, "Watcher:EpochGate:V1Put:Post");
    assert_eq!(find_post_uri(&user_id, &post_id).await, uri_before);
    assert_eq!(after.indexed_at, before.indexed_at);
    assert!(
        find_post_details(&user_id, &new_id).await.is_err(),
        "a v0 user's social/v1 post must not be indexed"
    );
    for path in [&copy_path, &new_path] {
        assert!(
            !event_line_stored(&line("PUT", &user_id, path)).await?,
            "an ignored event must store no event line"
        );
        assert!(!queued_for_retry(&uri(&user_id, path)).await?);
    }

    // Ignored as well
    test.del(&user_kp, &copy_path).await?;
    test.del(&user_kp, &new_path).await?;
    test.cleanup_post(&user_kp, &post_path).await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// A `social/v1` DEL of a user at `v0` is ignored: a DEL of the v1 copy of a post indexed from
/// `pubky.app` leaves the post in place.
#[tokio_shared_rt::test(shared)]
async fn test_v0_user_social_v1_post_del_is_ignored() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(
            &user_kp,
            &test_user("Watcher:EpochGate:V1Del", "epoch_gate"),
        )
        .await?;
    let (post_id, post_path) = test
        .create_post(&user_kp, &short_post("Watcher:EpochGate:V1Del:Post"))
        .await?;
    let copy_path = test
        .put_v1_post_version(
            &user_kp,
            &post_id,
            &post_id,
            &v1_note("Watcher:EpochGate:V1Del:Post"),
        )
        .await?;

    test.del(&user_kp, &copy_path).await?;

    let post = find_post_details(&user_id, &post_id).await?;
    assert_eq!(post.content, "Watcher:EpochGate:V1Del:Post");
    assert!(
        !post.deleted,
        "a v0 user's social/v1 DEL must not delete the post"
    );
    assert!(
        !event_line_stored(&line("DEL", &user_id, &copy_path)).await?,
        "an ignored event must store no event line"
    );
    assert!(!queued_for_retry(&uri(&user_id, &copy_path)).await?);

    test.cleanup_post(&user_kp, &post_path).await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// The cached details without `indexed_at`, which every profile write renews.
async fn cached_details(user_id: &str) -> Result<serde_json::Value> {
    let mut user = serde_json::to_value(find_cached_user_details(user_id).await?)?;
    user.as_object_mut()
        .expect("details serialize as an object")
        .remove("indexed_at");
    Ok(user)
}

/// The `social/v1` profile address of `user_id`.
fn v1_profile_uri(user_id: &str) -> String {
    format!("pubky://{user_id}/pub/social/v1/profile.json")
}

/// A `social/v1` profile PUT of a user at `v0` flips them: the profile is indexed with its v1
/// address, the user is at `v1` in the graph and the cache, and their `pubky.app` post stays.
/// Writing the same profile again changes nothing.
#[tokio_shared_rt::test(shared)]
async fn test_v0_user_flips_on_social_v1_profile_put() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(&user_kp, &test_user("Watcher:EpochGate:Flip", "epoch_gate"))
        .await?;
    let (post_id, post_path) = test
        .create_post(&user_kp, &short_post("Watcher:EpochGate:Flip:Post"))
        .await?;

    let profile = v1_test_user("Watcher:EpochGate:Flip:V1", "epoch_gate");
    let profile_path = test.put_v1_profile(&user_kp, &profile).await?;

    assert_eq!(find_user_epoch(&user_id).await.as_deref(), Some("v1"));
    let user = find_cached_user_details(&user_id).await?;
    assert_eq!(user.epoch, Epoch::V1);
    assert_eq!(user.name, "Watcher:EpochGate:Flip:V1");
    assert_eq!(
        find_user_uri(&user_id).await,
        Some(v1_profile_uri(&user_id))
    );
    assert!(event_line_stored(&line("PUT", &user_id, &profile_path)).await?);
    assert_eq!(
        find_post_details(&user_id, &post_id).await?.content,
        "Watcher:EpochGate:Flip:Post"
    );

    // The same profile again: a profile edit that changes nothing
    let before = cached_details(&user_id).await?;
    test.put_v1_profile(&user_kp, &profile).await?;
    assert_eq!(cached_details(&user_id).await?, before);
    assert_eq!(find_user_epoch(&user_id).await.as_deref(), Some("v1"));
    assert_eq!(
        find_user_uri(&user_id).await,
        Some(v1_profile_uri(&user_id))
    );

    // Back to `v0`, so the gate lets the `pubky.app` cleanup through; the v1 DEL is then ignored
    set_user_epoch(&user_id, "v0").await?;
    test.cleanup_post(&user_kp, &post_path).await?;
    test.del(&user_kp, &profile_path).await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// A user whose first event is a `social/v1` profile PUT is created at `v1`.
#[tokio_shared_rt::test(shared)]
async fn test_new_user_social_v1_profile_put_creates_the_user_at_v1() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = user_kp.public_key().to_z32();
    test.register_user(&user_kp).await?;
    assert!(find_user_details(&user_id).await.is_err(), "no node yet");

    let profile_path = test
        .put_v1_profile(
            &user_kp,
            &v1_test_user("Watcher:EpochGate:NewV1", "epoch_gate"),
        )
        .await?;

    assert_eq!(find_user_epoch(&user_id).await.as_deref(), Some("v1"));
    let user = find_cached_user_details(&user_id).await?;
    assert_eq!(user.epoch, Epoch::V1);
    assert_eq!(user.name, "Watcher:EpochGate:NewV1");
    assert_eq!(
        find_user_uri(&user_id).await,
        Some(v1_profile_uri(&user_id))
    );

    // Handled at `v1`
    test.del(&user_kp, &profile_path).await?;
    Ok(())
}

/// After the flip, the user is read through `social/v1` only: a profile edit is indexed and
/// keeps `v1`, a `pubky.app` post is ignored and a `social/v1` post is indexed.
#[tokio_shared_rt::test(shared)]
async fn test_flipped_user_is_read_through_social_v1() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(
            &user_kp,
            &test_user("Watcher:EpochGate:Flipped", "epoch_gate"),
        )
        .await?;
    let profile_path = test
        .put_v1_profile(
            &user_kp,
            &v1_test_user("Watcher:EpochGate:Flipped:V1", "epoch_gate"),
        )
        .await?;

    test.put_v1_profile(
        &user_kp,
        &v1_test_user("Watcher:EpochGate:Flipped:Edited", "epoch_gate"),
    )
    .await?;
    let user = find_cached_user_details(&user_id).await?;
    assert_eq!(user.name, "Watcher:EpochGate:Flipped:Edited");
    assert_eq!(user.epoch, Epoch::V1);
    assert_eq!(find_user_epoch(&user_id).await.as_deref(), Some("v1"));

    let (v0_post_id, v0_post_path) = test
        .create_post(&user_kp, &short_post("Watcher:EpochGate:Flipped:V0Post"))
        .await?;
    assert!(
        find_post_details(&user_id, &v0_post_id).await.is_err(),
        "a flipped user's pubky.app post must not be indexed"
    );

    let post_id = generate_post_id();
    let version_path = test
        .put_v1_post_version(
            &user_kp,
            &post_id,
            &post_id,
            &v1_note("Watcher:EpochGate:Flipped:V1Post"),
        )
        .await?;
    assert_eq!(
        find_post_details(&user_id, &post_id).await?.content,
        "Watcher:EpochGate:Flipped:V1Post"
    );

    // Handled at `v1`
    test.del(&user_kp, &version_path).await?;
    // Ignored at `v1`
    test.cleanup_post(&user_kp, &v0_post_path).await?;
    // Back to `v0`, so the gate lets the `pubky.app` cleanup through; the v1 DEL is then ignored
    set_user_epoch(&user_id, "v0").await?;
    test.del(&user_kp, &profile_path).await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// A flipped user who deletes their `social/v1` profile while a post remains keeps `v1` in
/// the tombstone and in the cache.
#[tokio_shared_rt::test(shared)]
async fn test_flipped_user_tombstone_keeps_v1() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(
            &user_kp,
            &test_user("Watcher:EpochGate:Tombstone", "epoch_gate"),
        )
        .await?;
    let profile_path = test
        .put_v1_profile(
            &user_kp,
            &v1_test_user("Watcher:EpochGate:Tombstone:V1", "epoch_gate"),
        )
        .await?;
    let post_id = generate_post_id();
    let version_path = test
        .put_v1_post_version(
            &user_kp,
            &post_id,
            &post_id,
            &v1_note("Watcher:EpochGate:Tombstone:Post"),
        )
        .await?;

    test.del(&user_kp, &profile_path).await?;

    let tombstone = find_user_details(&user_id).await?;
    assert!(tombstone.deleted);
    assert_eq!(tombstone.epoch, Epoch::V1);
    let cached = find_cached_user_details(&user_id).await?;
    assert!(cached.deleted);
    assert_eq!(cached.epoch, Epoch::V1);

    // Handled at `v1`
    test.del(&user_kp, &version_path).await?;
    // Back to `v0`, so the gate lets the `pubky.app` cleanup through
    set_user_epoch(&user_id, "v0").await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// A `social/v1` post PUT of a user at `v1` is indexed, and its DEL deletes the post.
#[tokio_shared_rt::test(shared)]
async fn test_v1_user_social_v1_post_is_indexed() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(
            &user_kp,
            &test_user("Watcher:EpochGate:V1User", "epoch_gate"),
        )
        .await?;
    set_user_epoch(&user_id, "v1").await?;

    let post_id = generate_post_id();
    let version_path = test
        .put_v1_post_version(
            &user_kp,
            &post_id,
            &post_id,
            &v1_note("Watcher:EpochGate:V1User:Post"),
        )
        .await?;

    let post = find_post_details(&user_id, &post_id).await?;
    assert_eq!(post.content, "Watcher:EpochGate:V1User:Post");
    assert_eq!(
        find_post_uri(&user_id, &post_id).await,
        Some(format!("pubky://{user_id}/pub/social/v1/posts/{post_id}"))
    );
    assert!(event_line_stored(&line("PUT", &user_id, &version_path)).await?);

    test.del(&user_kp, &version_path).await?;
    assert!(
        find_post_details(&user_id, &post_id).await.is_err(),
        "a v1 user's social/v1 DEL must delete the post"
    );

    // Back to `v0`, so the gate lets the cleanup through
    set_user_epoch(&user_id, "v0").await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// A `social/v1` edit of a post first indexed from `pubky.app`, by a user since moved to `v1`,
/// lands on the same post node and keeps its `indexed_at`, so the post doesn't move in feeds.
#[tokio_shared_rt::test(shared)]
async fn test_v1_user_social_v1_edit_of_a_pubky_app_post_keeps_indexed_at() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(
            &user_kp,
            &test_user("Watcher:EpochGate:V1Edit", "epoch_gate"),
        )
        .await?;
    let (post_id, post_path) = test
        .create_post(&user_kp, &short_post("Watcher:EpochGate:V1Edit:Post"))
        .await?;
    let before = find_post_details(&user_id, &post_id).await?;
    set_user_epoch(&user_id, "v1").await?;

    let edit_id = generate_post_id();
    let version_path = test
        .put_v1_post_version(
            &user_kp,
            &post_id,
            &edit_id,
            &v1_note("Watcher:EpochGate:V1Edit:Edited"),
        )
        .await?;

    let after = find_post_details(&user_id, &post_id).await?;
    assert_eq!(after.content, "Watcher:EpochGate:V1Edit:Edited");
    assert_eq!(after.id, before.id);
    assert_eq!(after.indexed_at, before.indexed_at);

    // Back to `v0`, so the gate lets the `pubky.app` cleanup through; the v1 DEL is then ignored
    set_user_epoch(&user_id, "v0").await?;
    test.cleanup_post(&user_kp, &post_path).await?;
    test.del(&user_kp, &version_path).await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// A `social/v1` follow PUT of a user at `v1` creates the `FOLLOWS` edge, and its DEL removes
/// it.
#[tokio_shared_rt::test(shared)]
async fn test_v1_user_social_v1_follow_is_indexed() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let follower_kp = Keypair::random();
    let follower_id = test
        .create_user(
            &follower_kp,
            &test_user("Watcher:EpochGate:V1Follow:Follower", "epoch_gate"),
        )
        .await?;
    let followee_kp = Keypair::random();
    let followee_id = test
        .create_user(
            &followee_kp,
            &test_user("Watcher:EpochGate:V1Follow:Followee", "epoch_gate"),
        )
        .await?;
    set_user_epoch(&follower_id, "v1").await?;

    let follow_path = test.put_v1_follow(&follower_kp, &followee_id).await?;
    assert!(
        find_follow_relationship(&follower_id, &followee_id).await?,
        "a v1 user's social/v1 follow must create the edge"
    );
    assert!(event_line_stored(&line("PUT", &follower_id, &follow_path)).await?);

    test.del(&follower_kp, &follow_path).await?;
    assert!(
        !find_follow_relationship(&follower_id, &followee_id).await?,
        "a v1 user's social/v1 follow DEL must remove the edge"
    );

    // Back to `v0`, so the gate lets the cleanup through
    set_user_epoch(&follower_id, "v0").await?;
    test.cleanup_user(&follower_kp).await?;
    test.cleanup_user(&followee_kp).await?;
    Ok(())
}

/// A `social/v1` tag PUT of a user at `v1` on a post, named by its v1 address, creates the
/// `TAGGED` edge, and its DEL removes it.
#[tokio_shared_rt::test(shared)]
async fn test_v1_user_social_v1_post_tag_is_indexed() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:EpochGate:V1Tag:Author", "epoch_gate"),
        )
        .await?;
    let tagger_kp = Keypair::random();
    let tagger_id = test
        .create_user(
            &tagger_kp,
            &test_user("Watcher:EpochGate:V1Tag:Tagger", "epoch_gate"),
        )
        .await?;
    set_user_epoch(&author_id, "v1").await?;
    set_user_epoch(&tagger_id, "v1").await?;

    let post_id = generate_post_id();
    let version_path = test
        .put_v1_post_version(
            &author_kp,
            &post_id,
            &post_id,
            &v1_note("Watcher:EpochGate:V1Tag:Post"),
        )
        .await?;

    let label = "epochgate";
    let tag = PubkySocialTag::new(
        v1_post_uri_builder(author_id.clone(), post_id.clone()),
        label.to_string(),
    );
    let tag_path = test.put_v1_tag(&tagger_kp, &tag).await?;
    let post_tag = find_post_tag(&author_id, &post_id, label)
        .await?
        .expect("a v1 user's social/v1 tag must tag the post");
    assert_eq!(post_tag.taggers_count, 1);
    assert_eq!(post_tag.taggers, [tagger_id.as_str()]);
    assert!(event_line_stored(&line("PUT", &tagger_id, &tag_path)).await?);

    test.del(&tagger_kp, &tag_path).await?;
    assert!(
        find_post_tag(&author_id, &post_id, label).await?.is_none(),
        "a v1 user's social/v1 tag DEL must remove the tag"
    );

    // Handled at `v1`
    test.del(&author_kp, &version_path).await?;
    // Back to `v0`, so the gate lets the cleanup through
    set_user_epoch(&author_id, "v0").await?;
    set_user_epoch(&tagger_id, "v0").await?;
    test.cleanup_user(&tagger_kp).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}
