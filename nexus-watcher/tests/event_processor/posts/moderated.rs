use crate::event_processor::{
    posts::utils::{find_post_details, short_post, test_user},
    tags::utils::check_member_post_tag_global_timeline,
    users::utils::find_user_counts,
    utils::default_moderation_tests,
    utils::watcher::{generate_post_id, HomeserverHashIdPath, WatcherTest},
};
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::notification::Notification;
use nexus_common::models::post::PostCounts;
use nexus_common::models::takedown::record_takedown;
use nexus_common::types::Pagination;
use nexus_common::utils::test_utils::default_ingestor_tests;
use nexus_watcher::events::inputs::{Reference, ReferenceTarget, ResourceKey};
use nexus_watcher::events::remove_if_taken_down;
use nexus_watcher::EventProcessorError;
use pubky::ResourcePath;
use pubky::{recovery_file, Keypair};
use pubky_social_specs::legacy_v0::traits::HasIdPath;
use pubky_social_specs::legacy_v0::{
    post_uri_builder, PubkyAppPost, PubkyAppPostKind, PubkyAppTag, PubkyAppUser,
};
use pubky_social_specs::{post_uri_builder as social_v1_post_uri_builder, PubkyId};
use std::path::Path;
use tokio::fs;

const MODERATION_LABEL: &str = "label_to_moderate";

/// The reference a moderator tag written as `uri` resolves to: the post `post_id` by `owner_id`.
fn post_reference(uri: &str, owner_id: &str, post_id: &str) -> Reference {
    Reference {
        uri: uri.to_string(),
        target: ReferenceTarget::Post(ResourceKey {
            owner_id: PubkyId::try_from(owner_id).expect("a valid user id"),
            resource_id: post_id.to_string(),
        }),
    }
}
/// The address of a moderator tag applied directly in a test, kept with its takedown.
const MODERATOR_TAG_URI: &str =
    "pubky://uo7jgkykft4885n8cruizwy6khw71mnu5pq3ay9i8pw1ymcn85ko/pub/pubky.app/tags/MODERATORTAG";

#[tokio_shared_rt::test(shared)]
async fn test_moderated_post_lifecycle() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    // 1. User signup and writes a post
    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_homeserver_post_to_moderate".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostModerate:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    let post = PubkyAppPost {
        content: "Watcher:PostModerate:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
        lock: None,
    };

    let (post_id, _post_path) = test.create_post(&user_kp, &post).await?;

    // 2. Confirm this post does exist
    let post_details = find_post_details(&user_id, &post_id).await.unwrap();
    assert_eq!(post_details.id, post_id);

    // 3. Load moderation service key and place a tag on that post with label "label_to_moderate"
    let moderator_recovery_file = fs::read("./tests/event_processor/utils/moderator_key.pkarr")
        .await
        .unwrap();
    let moderator_key =
        recovery_file::decrypt_recovery_file(&moderator_recovery_file, "password").unwrap();

    test.create_user(&moderator_key, &user).await?;

    let tag = PubkyAppTag {
        uri: post_uri_builder(user_id.clone(), post_id.clone()),
        label: "label_to_moderate".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();
    // Put tag
    test.put(&moderator_key, &tag_path, tag).await?;

    // 4. Confirm the post does not exist
    let post_details = find_post_details(&user_id, &post_id).await;
    assert!(post_details.is_err());

    Ok(())
}

/// Moderating a post that still has a tag from another user must tombstone the
/// post (cleared and flagged `deleted`) instead of DETACH DELETEing the node,
/// so the tagger's counts and the tag timeline index stay consistent.
#[tokio_shared_rt::test(shared)]
async fn test_moderated_post_with_tag_is_tombstoned() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    // Author signup and post creation
    let author_kp = Keypair::random();
    let author = test_user("Watcher:PostModerateTagged:Author", "author");
    let author_id = test.create_user(&author_kp, &author).await?;

    let post = short_post("Watcher:PostModerateTagged:Post");
    let (post_id, _post_path) = test.create_post(&author_kp, &post).await?;

    // A second user tags the post
    let tagger_kp = Keypair::random();
    let tagger = test_user("Watcher:PostModerateTagged:Tagger", "tagger");
    let tagger_id = test.create_user(&tagger_kp, &tagger).await?;

    let label = "mod_tagged_post";
    let tag = PubkyAppTag {
        uri: post_uri_builder(author_id.clone(), post_id.clone()),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();
    test.put(&tagger_kp, &tag_path, tag).await?;

    // Sanity: the tag is indexed before moderation
    let post_key: [&str; 2] = [&author_id, &post_id];
    assert_eq!(
        find_user_counts(&tagger_id).await.tagged,
        1,
        "tagger's tagged count should be 1 before moderation"
    );
    assert!(
        check_member_post_tag_global_timeline(&post_key, label)
            .await?
            .is_some(),
        "tag timeline entry should exist before moderation"
    );

    // Moderator flags the post
    let moderator_kp = create_moderator(&mut test).await?;
    moderate_post(&mut test, &moderator_kp, &author_id, &post_id).await?;

    // The post node survives as a tombstone with its content cleared
    let post_details = find_post_details(&author_id, &post_id)
        .await
        .expect("moderated post with edges should still exist as a tombstone");
    assert!(
        post_details.deleted,
        "moderated post should be flagged deleted"
    );
    assert_eq!(
        post_details.content, "",
        "moderated post content should be cleared"
    );

    // The tagger's state stays intact: counts and tag timeline are untouched
    assert_eq!(
        find_user_counts(&tagger_id).await.tagged,
        1,
        "tagger's tagged count should stay intact after moderation"
    );
    assert!(
        check_member_post_tag_global_timeline(&post_key, label)
            .await?
            .is_some(),
        "tag timeline entry should stay intact after moderation"
    );

    // End-to-end kill shot: the tagger deletes their tag on the tombstoned post.
    // Because the TAGGED edge survived moderation, the DEL event runs the normal
    // cleanup path. With a DETACH DELETE (the old behavior) the edge is gone,
    // tag::del hits its idempotent no-op and the state below stays orphaned.
    test.del(&tagger_kp, &tag_path).await?;

    assert_eq!(
        find_user_counts(&tagger_id).await.tagged,
        0,
        "tagger's tagged count should drop to 0 once they delete their tag"
    );
    assert!(
        check_member_post_tag_global_timeline(&post_key, label)
            .await?
            .is_none(),
        "tag timeline entry should be gone once the tagger deletes their tag"
    );

    Ok(())
}

/// Moderating a post without any edges must still fully delete it from both
/// the graph and the Redis indexes, exactly as before the tombstone gating.
#[tokio_shared_rt::test(shared)]
async fn test_moderated_bare_post_is_hard_deleted() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    // Author signup and post creation
    let author_kp = Keypair::random();
    let author = test_user("Watcher:PostModerateBare:Author", "author");
    let author_id = test.create_user(&author_kp, &author).await?;

    let post = short_post("Watcher:PostModerateBare:Post");
    let (post_id, _post_path) = test.create_post(&author_kp, &post).await?;

    // Sanity: the post exists before moderation
    assert!(find_post_details(&author_id, &post_id).await.is_ok());

    // Moderator flags the post
    let moderator_kp = create_moderator(&mut test).await?;
    moderate_post(&mut test, &moderator_kp, &author_id, &post_id).await?;

    // The edge-free post is hard-deleted from the graph, not tombstoned
    assert!(
        find_post_details(&author_id, &post_id).await.is_err(),
        "bare moderated post should be fully deleted from the graph"
    );

    // The cached counts are gone as well
    let post_counts = PostCounts::get_from_index(&author_id, &post_id)
        .await
        .unwrap();
    assert!(
        post_counts.is_none(),
        "bare moderated post counts should be deleted from the index"
    );

    Ok(())
}

async fn create_moderator(test: &mut WatcherTest) -> Result<Keypair> {
    let moderator_recovery_file =
        fs::read("./tests/event_processor/utils/moderator_key.pkarr").await?;
    let moderator_kp = recovery_file::decrypt_recovery_file(&moderator_recovery_file, "password")?;
    let moderator = test_user("Watcher:PostModerate:Moderator", "moderator");
    test.create_user(&moderator_kp, &moderator).await?;
    Ok(moderator_kp)
}

async fn moderate_post(
    test: &mut WatcherTest,
    moderator_kp: &Keypair,
    author_id: &str,
    post_id: &str,
) -> Result<()> {
    let tag = PubkyAppTag {
        uri: post_uri_builder(author_id.to_string(), post_id.to_string()),
        label: MODERATION_LABEL.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();
    test.put(moderator_kp, &tag_path, tag).await
}

/// A takedown read before the post it targets keeps that post out: a new Nexus reads
/// everyone's history in no particular order, so the moderator's tag can come first.
#[tokio_shared_rt::test(shared)]
async fn test_post_taken_down_before_it_is_read_stays_out() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Watcher:PostModerate:Early".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    let post_id = generate_post_id();
    let post_path: ResourcePath = PubkyAppPost::create_path(&post_id).parse()?;
    let tag = PubkyAppTag {
        uri: post_uri_builder(user_id.clone(), post_id.clone()),
        label: MODERATION_LABEL.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    // The moderator's tag is read first, while there is no post to delete: deleting
    // reports the post as unindexed, as it did before takedowns were remembered.
    let result = default_moderation_tests()
        .apply_moderation(
            &tag.label,
            post_reference(&tag.uri, &user_id, &post_id),
            MODERATOR_TAG_URI,
            Path::new("/tmp/nexus-watcher-test"),
            &default_ingestor_tests(),
        )
        .await;
    assert!(
        matches!(result, Err(EventProcessorError::SkipIndexing)),
        "{result:?}"
    );

    let post = PubkyAppPost {
        content: "Watcher:PostModerate:Early:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
        lock: None,
    };
    test.put(&user_kp, &post_path, post).await?;

    assert!(
        find_post_details(&user_id, &post_id).await.is_err(),
        "the taken-down post stays out when its event comes later"
    );
    Ok(())
}

/// A post written while the moderator's tag was being read elsewhere (another loop, the
/// fetch in flight) doesn't stay: once written, a recorded takedown removes it.
#[tokio_shared_rt::test(shared)]
async fn test_post_written_after_its_takedown_was_recorded_is_removed() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Watcher:PostModerate:Race".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;
    let post = PubkyAppPost {
        content: "Watcher:PostModerate:Race:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
        lock: None,
    };
    // The write that passed the first check before the takedown was recorded.
    let (post_id, _) = test.create_post(&user_kp, &post).await?;
    assert!(find_post_details(&user_id, &post_id).await.is_ok());

    // The takedown, recorded by another loop that found nothing to delete yet.
    let post_uri = post_uri_builder(user_id.clone(), post_id.clone());
    record_takedown(&post_uri, MODERATOR_TAG_URI).await?;

    remove_if_taken_down(
        &post_uri,
        Path::new("/tmp/nexus-watcher-test"),
        &default_ingestor_tests(),
    )
    .await?;

    assert!(
        find_post_details(&user_id, &post_id).await.is_err(),
        "the post written before the takedown was recorded doesn't stay"
    );
    Ok(())
}

/// A takedown is looked up by the record an address resolves to: a post taken down under its
/// `pubky.app` address is found through its `social/v1` address too. The classifier still
/// skips `social/v1` events, so the lookup is called directly.
#[tokio_shared_rt::test(shared)]
async fn test_post_takedown_holds_for_its_social_v1_address() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let user_kp = Keypair::random();
    let user_id = test
        .create_user(&user_kp, &test_user("Watcher:PostModerate:V1Lookup", ""))
        .await?;
    let (post_id, _) = test
        .create_post(&user_kp, &short_post("Watcher:PostModerate:V1Lookup:Post"))
        .await?;
    assert!(find_post_details(&user_id, &post_id).await.is_ok());

    record_takedown(
        &post_uri_builder(user_id.clone(), post_id.clone()),
        MODERATOR_TAG_URI,
    )
    .await?;

    let removed = remove_if_taken_down(
        &social_v1_post_uri_builder(user_id.clone(), post_id.clone()),
        Path::new("/tmp/nexus-watcher-test"),
        &default_ingestor_tests(),
    )
    .await?;

    assert!(removed, "the social/v1 address finds the post's takedown");
    assert!(
        find_post_details(&user_id, &post_id).await.is_err(),
        "the taken-down post is removed"
    );
    Ok(())
}

/// A user or an address that resolves to nothing Nexus takes down has no takedown address, so
/// its takedown is looked up by the address as written: another spelling of it misses.
#[tokio_shared_rt::test(shared)]
async fn test_takedown_of_a_user_or_unresolved_address_is_looked_up_as_written() -> Result<()> {
    let _test = WatcherTest::setup(None).await?;
    let user_id = Keypair::random().public_key().to_z32();
    let other_id = Keypair::random().public_key().to_z32();
    let files_path = Path::new("/tmp/nexus-watcher-test");
    let ingestor = default_ingestor_tests();

    for (recorded, other_spelling) in [
        // A user: its profile address and its bare key resolve to the same user.
        (
            format!("pubky://{user_id}/pub/pubky.app/profile.json"),
            format!("pubky://{user_id}"),
        ),
        // Unresolved: a follow, and a URL.
        (
            format!("pubky://{user_id}/pub/pubky.app/follows/{other_id}"),
            format!("pubky://{user_id}/pub/pubky.app/follows/{other_id}/"),
        ),
        (
            format!("https://example.com/takedown-fallback/{user_id}"),
            format!("https://example.com/takedown-fallback/{user_id}/"),
        ),
    ] {
        assert!(
            !remove_if_taken_down(&recorded, files_path, &ingestor).await?,
            "{recorded} is not taken down yet"
        );
        record_takedown(&recorded, MODERATOR_TAG_URI).await?;
        assert!(
            remove_if_taken_down(&recorded, files_path, &ingestor).await?,
            "{recorded} is looked up as written"
        );
        assert!(
            !remove_if_taken_down(&other_spelling, files_path, &ingestor).await?,
            "{other_spelling} is another address"
        );
    }
    Ok(())
}

/// A moderator tag naming the post with a non-canonical address (a trailing slash) is
/// recorded under the address the post's event carries, so the post stays out when its
/// event comes later.
#[tokio_shared_rt::test(shared)]
async fn test_post_taken_down_with_a_trailing_slash_stays_out() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let user_kp = Keypair::random();
    let user_id = test
        .create_user(&user_kp, &test_user("Watcher:PostModerate:Slash", ""))
        .await?;

    let post_id = generate_post_id();
    let post_path: ResourcePath = PubkyAppPost::create_path(&post_id).parse()?;
    let tag = PubkyAppTag {
        uri: format!("{}/", post_uri_builder(user_id.clone(), post_id.clone())),
        label: MODERATION_LABEL.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let _ = default_moderation_tests()
        .apply_moderation(
            &tag.label,
            post_reference(&tag.uri, &user_id, &post_id),
            MODERATOR_TAG_URI,
            Path::new("/tmp/nexus-watcher-test"),
            &default_ingestor_tests(),
        )
        .await;

    test.put(
        &user_kp,
        &post_path,
        short_post("Watcher:PostModerate:Slash:Post"),
    )
    .await?;

    assert!(
        find_post_details(&user_id, &post_id).await.is_err(),
        "taken down under its canonical address, so its event is dropped"
    );
    Ok(())
}

/// A moderator tag naming the post by its `social/v1` address is recorded under the
/// `pubky.app` address the post's own event carries, so the post stays out when its event
/// comes later.
#[tokio_shared_rt::test(shared)]
async fn test_post_taken_down_through_its_social_v1_address_stays_out() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let user_kp = Keypair::random();
    let user_id = test
        .create_user(&user_kp, &test_user("Watcher:PostModerate:SocialV1", ""))
        .await?;

    let post_id = generate_post_id();
    let post_path: ResourcePath = PubkyAppPost::create_path(&post_id).parse()?;
    let tag = PubkyAppTag {
        uri: social_v1_post_uri_builder(user_id.clone(), post_id.clone()),
        label: MODERATION_LABEL.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let _ = default_moderation_tests()
        .apply_moderation(
            &tag.label,
            post_reference(&tag.uri, &user_id, &post_id),
            MODERATOR_TAG_URI,
            Path::new("/tmp/nexus-watcher-test"),
            &default_ingestor_tests(),
        )
        .await;

    test.put(
        &user_kp,
        &post_path,
        short_post("Watcher:PostModerate:SocialV1:Post"),
    )
    .await?;

    assert!(
        find_post_details(&user_id, &post_id).await.is_err(),
        "taken down under its pubky.app address, so its event is dropped"
    );
    Ok(())
}

/// A reply taken down before it is read is never written, so the post it replies to
/// leaves its author no notification: not a new reply, nor a deleted one.
#[tokio_shared_rt::test(shared)]
async fn test_reply_taken_down_before_it_is_read_notifies_no_one() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let author_kp = Keypair::random();
    let author_id = test
        .create_user(&author_kp, &test_user("Watcher:PostModerate:Parent", ""))
        .await?;
    let (parent_id, _) = test
        .create_post(&author_kp, &short_post("Watcher:PostModerate:Parent:Post"))
        .await?;
    let replier_kp = Keypair::random();
    let replier_id = test
        .create_user(&replier_kp, &test_user("Watcher:PostModerate:Replier", ""))
        .await?;

    let reply_id = generate_post_id();
    let reply_path: ResourcePath = PubkyAppPost::create_path(&reply_id).parse()?;
    let tag = PubkyAppTag {
        uri: post_uri_builder(replier_id.clone(), reply_id.clone()),
        label: MODERATION_LABEL.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let _ = default_moderation_tests()
        .apply_moderation(
            &tag.label,
            post_reference(&tag.uri, &replier_id, &reply_id),
            MODERATOR_TAG_URI,
            Path::new("/tmp/nexus-watcher-test"),
            &default_ingestor_tests(),
        )
        .await;

    let mut reply = short_post("Watcher:PostModerate:Reply");
    reply.parent = Some(post_uri_builder(author_id.clone(), parent_id));
    test.put(&replier_kp, &reply_path, reply).await?;

    let notifications = Notification::get_by_id(&author_id, Pagination::default()).await?;
    assert!(
        notifications.is_empty(),
        "nothing was written, so no one was notified: {notifications:?}"
    );
    Ok(())
}
