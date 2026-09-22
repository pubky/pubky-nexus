use super::utils::{assert_user_ingested, create_external_test_homeserver};
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::error::ModelError;
use nexus_common::models::user::{UserDetails, UserIngestor};
use nexus_common::utils::test_utils::random_pubky_id;
use nexus_watcher::events::handlers;
use nexus_watcher::EventProcessorError;
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder,
    traits::{HashId, TimestampId},
    user_uri_builder, PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind, PubkyAppTag, PubkyAppUser,
    PubkyId,
};

/// A [`UserIngestor`] whose blacklist contains the user's HS refuses to ingest
/// that user, returning [`ModelError::HsBlacklisted`] and leaving no graph node behind.
#[tokio_shared_rt::test(shared)]
async fn test_maybe_ingest_user_aborts_on_blacklisted_homeserver() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let hs_pk = create_external_test_homeserver(&mut test).await?;

    let user_kp = Keypair::random();
    let user_id = user_kp.public_key().to_z32();
    test.register_user_in_hs(&user_kp, &hs_pk).await?;

    let ingestor = UserIngestor::new([PubkyId::from(hs_pk.clone())]);
    let user_pubky_id = PubkyId::from(user_kp.public_key());

    let err = ingestor
        .maybe_ingest_user(&user_pubky_id)
        .await
        .expect_err("ingestion should be refused for a blacklisted HS");
    assert!(
        matches!(err, ModelError::HsBlacklisted { .. }),
        "expected HsBlacklisted, got {err:?}"
    );

    assert!(
        UserDetails::get_by_id(&user_id).await?.is_none(),
        "blacklisted user {user_id} must not be ingested"
    );

    Ok(())
}

/// Control: with an empty blacklist the same user is ingested normally, proving
/// the blacklist (not some other failure) is what blocked ingestion above.
#[tokio_shared_rt::test(shared)]
async fn test_maybe_ingest_user_ingests_when_not_blacklisted() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let hs_pk = create_external_test_homeserver(&mut test).await?;

    let user_kp = Keypair::random();
    let user_id = user_kp.public_key().to_z32();
    test.register_user_in_hs(&user_kp, &hs_pk).await?;

    let user_pubky_id = PubkyId::from(user_kp.public_key());
    UserIngestor::default()
        .maybe_ingest_user(&user_pubky_id)
        .await?;

    assert_user_ingested(&user_id, &hs_pk).await;

    Ok(())
}

/// An event depending on a user hosted by a blacklisted HS must fail with the
/// non-retryable [`EventProcessorError::HsBlacklisted`] instead of
/// `MissingDependency`, so the event is dropped instead of being retried
/// against a dependency that cannot resolve.
#[tokio_shared_rt::test(shared)]
async fn test_follow_of_user_on_blacklisted_homeserver_is_dropped() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let hs_pk = create_external_test_homeserver(&mut test).await?;

    let followee_kp = Keypair::random();
    test.register_user_in_hs(&followee_kp, &hs_pk).await?;

    let follower_kp = Keypair::random();
    let follower_user = PubkyAppUser {
        bio: Some("test_follow_of_user_on_blacklisted_homeserver".to_string()),
        image: None,
        links: None,
        name: "Watcher:UserIngestion:FollowBlacklisted".to_string(),
        status: None,
    };
    test.create_user(&follower_kp, &follower_user).await?;

    let ingestor = UserIngestor::new([PubkyId::from(hs_pk.clone())]);
    let err = handlers::follow::sync_put(
        PubkyId::from(follower_kp.public_key()),
        PubkyId::from(followee_kp.public_key()),
        &ingestor,
    )
    .await
    .expect_err("follow of a user on a blacklisted HS must fail");

    assert!(
        matches!(err, EventProcessorError::HsBlacklisted { .. }),
        "expected HsBlacklisted, got {err:?}"
    );

    Ok(())
}

/// A reply to a post whose author is hosted by a blacklisted HS must fail with
/// the non-retryable [`EventProcessorError::HsBlacklisted`] (not
/// `MissingDependency`), so it is dropped instead of retried, and the
/// blacklisted parent author is never ingested.
#[tokio_shared_rt::test(shared)]
async fn test_reply_to_post_on_blacklisted_homeserver_is_dropped() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let parent_hs_pk = create_external_test_homeserver(&mut test).await?;

    // Parent post author lives on the blacklisted HS and is never ingested.
    let parent_author_kp = Keypair::random();
    let parent_author_id = parent_author_kp.public_key().to_z32();
    test.register_user_in_hs(&parent_author_kp, &parent_hs_pk)
        .await?;

    let parent_post = PubkyAppPost {
        content: "Watcher:UserIngestion:ReplyBlacklisted:Parent".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
        lock: None,
    };
    let parent_post_uri = post_uri_builder(parent_author_id.clone(), parent_post.create_id());

    let reply = PubkyAppPost {
        content: "Watcher:UserIngestion:ReplyBlacklisted:Reply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: Some(parent_post_uri),
        embed: None,
        attachments: None,
        lock: None,
    };
    let reply_id = reply.create_id();

    let ingestor = UserIngestor::new([PubkyId::from(parent_hs_pk.clone())]);
    let err = handlers::post::sync_put(reply, random_pubky_id(), reply_id, &ingestor)
        .await
        .expect_err("reply to a post on a blacklisted HS must fail");

    assert!(
        matches!(err, EventProcessorError::HsBlacklisted { .. }),
        "expected HsBlacklisted, got {err:?}"
    );
    assert!(
        UserDetails::get_by_id(&parent_author_id).await?.is_none(),
        "blacklisted parent author {parent_author_id} must not be ingested"
    );

    Ok(())
}

/// A repost of a post whose author is hosted by a blacklisted HS must fail with
/// the non-retryable [`EventProcessorError::HsBlacklisted`] (not
/// `MissingDependency`), so it is dropped instead of retried, and the
/// blacklisted reposted author is never ingested.
#[tokio_shared_rt::test(shared)]
async fn test_repost_of_post_on_blacklisted_homeserver_is_dropped() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let original_hs_pk = create_external_test_homeserver(&mut test).await?;

    // Reposted post author lives on the blacklisted HS and is never ingested.
    let original_author_kp = Keypair::random();
    let original_author_id = original_author_kp.public_key().to_z32();
    test.register_user_in_hs(&original_author_kp, &original_hs_pk)
        .await?;

    let original_post = PubkyAppPost {
        content: "Watcher:UserIngestion:RepostBlacklisted:Original".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
        lock: None,
    };
    let original_post_uri = post_uri_builder(original_author_id.clone(), original_post.create_id());

    let repost = PubkyAppPost {
        content: "Watcher:UserIngestion:RepostBlacklisted:Repost".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: Some(PubkyAppPostEmbed {
            kind: PubkyAppPostKind::Short,
            uri: original_post_uri,
        }),
        attachments: None,
        lock: None,
    };
    let repost_id = repost.create_id();

    let ingestor = UserIngestor::new([PubkyId::from(original_hs_pk.clone())]);
    let err = handlers::post::sync_put(repost, random_pubky_id(), repost_id, &ingestor)
        .await
        .expect_err("repost of a post on a blacklisted HS must fail");

    assert!(
        matches!(err, EventProcessorError::HsBlacklisted { .. }),
        "expected HsBlacklisted, got {err:?}"
    );
    assert!(
        UserDetails::get_by_id(&original_author_id).await?.is_none(),
        "blacklisted reposted author {original_author_id} must not be ingested"
    );

    Ok(())
}

/// A tag on a post whose author is hosted by a blacklisted HS must fail with the
/// non-retryable [`EventProcessorError::HsBlacklisted`] (not `MissingDependency`),
/// so it is dropped instead of retried, and the blacklisted post author is never
/// ingested.
#[tokio_shared_rt::test(shared)]
async fn test_tag_post_on_blacklisted_homeserver_is_dropped() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let post_hs_pk = create_external_test_homeserver(&mut test).await?;

    // Tagged post author lives on the blacklisted HS and is never ingested.
    let post_author_kp = Keypair::random();
    let post_author_id = post_author_kp.public_key().to_z32();
    test.register_user_in_hs(&post_author_kp, &post_hs_pk)
        .await?;

    let post = PubkyAppPost {
        content: "Watcher:UserIngestion:TagPostBlacklisted:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
        lock: None,
    };
    let post_uri = post_uri_builder(post_author_id.clone(), post.create_id());

    let tag = PubkyAppTag {
        uri: post_uri,
        label: "test".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_id = tag.create_id();

    let ingestor = UserIngestor::new([PubkyId::from(post_hs_pk.clone())]);
    let err = handlers::tag::sync_put(tag, random_pubky_id(), tag_id, &ingestor)
        .await
        .expect_err("tag on a post hosted by a blacklisted HS must fail");

    assert!(
        matches!(err, EventProcessorError::HsBlacklisted { .. }),
        "expected HsBlacklisted, got {err:?}"
    );
    assert!(
        UserDetails::get_by_id(&post_author_id).await?.is_none(),
        "blacklisted post author {post_author_id} must not be ingested"
    );

    Ok(())
}

/// A tag on a user hosted by a blacklisted HS must fail with the non-retryable
/// [`EventProcessorError::HsBlacklisted`] (not `MissingDependency`), so it is
/// dropped instead of retried, and the blacklisted tagged user is never ingested.
#[tokio_shared_rt::test(shared)]
async fn test_tag_user_on_blacklisted_homeserver_is_dropped() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let tagged_hs_pk = create_external_test_homeserver(&mut test).await?;

    // Tagged user lives on the blacklisted HS and is never ingested.
    let tagged_user_kp = Keypair::random();
    let tagged_user_id = tagged_user_kp.public_key().to_z32();
    test.register_user_in_hs(&tagged_user_kp, &tagged_hs_pk)
        .await?;

    let tag = PubkyAppTag {
        uri: user_uri_builder(tagged_user_id.clone()),
        label: "test".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_id = tag.create_id();

    let ingestor = UserIngestor::new([PubkyId::from(tagged_hs_pk.clone())]);
    let err = handlers::tag::sync_put(tag, random_pubky_id(), tag_id, &ingestor)
        .await
        .expect_err("tag on a user hosted by a blacklisted HS must fail");

    assert!(
        matches!(err, EventProcessorError::HsBlacklisted { .. }),
        "expected HsBlacklisted, got {err:?}"
    );
    assert!(
        UserDetails::get_by_id(&tagged_user_id).await?.is_none(),
        "blacklisted tagged user {tagged_user_id} must not be ingested"
    );

    Ok(())
}
