use super::utils::create_external_test_homeserver;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::user::{UserCounts, UserIngestor};
use nexus_watcher::events::handlers;
use nexus_watcher::EventProcessorError;
use pubky::Keypair;
use pubky_app_specs::{
    tag_uri_builder, traits::HashId, user_uri_builder, PubkyAppTag, PubkyAppUser, PubkyId,
};

/// A user known to the follower's homeserver but not to Nexus, so the first event
/// that references them ingests them as a stub.
async fn unknown_user(test: &mut WatcherTest) -> Result<(String, PubkyId)> {
    let hs_pk = create_external_test_homeserver(test).await?;
    let kp = Keypair::random();
    test.register_user_in_hs(&kp, &hs_pk).await?;
    Ok((kp.public_key().to_z32(), PubkyId::from(kp.public_key())))
}

async fn indexed_user(test: &mut WatcherTest, name: &str) -> Result<(String, PubkyId)> {
    let kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some(name.to_string()),
        image: None,
        links: None,
        name: format!("Watcher:StubCounts:{name}"),
        status: None,
    };
    let id = test.create_user(&kp, &user).await?;
    Ok((id, PubkyId::from(kp.public_key())))
}

async fn counts_of(user_id: &str) -> Result<UserCounts> {
    Ok(UserCounts::get_from_index(user_id)
        .await?
        .unwrap_or_else(|| panic!("{user_id} should have a counts document")))
}

/// Follows a user Nexus does not know yet: the first attempt ingests them as a stub,
/// the retry lands the follow on that stub.
async fn follow_unknown_user(test: &mut WatcherTest) -> Result<(String, PubkyId)> {
    let (_, follower) = indexed_user(test, "Follower").await?;
    let (followee_id, followee) = unknown_user(test).await?;
    let ingestor = UserIngestor::default();

    let first = handlers::follow::sync_put(follower.clone(), followee.clone(), &ingestor).await;
    assert!(
        matches!(first, Err(EventProcessorError::MissingDependency { .. })),
        "the first follow should wait for the followee: {first:?}"
    );
    handlers::follow::sync_put(follower, followee.clone(), &ingestor).await?;

    Ok((followee_id, followee))
}

/// A stub user gets a counts document when it is created, so a follow that reaches it
/// before its profile is counted instead of skipped.
#[tokio_shared_rt::test(shared)]
async fn test_follow_on_stub_user_is_counted() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let (followee_id, _) = follow_unknown_user(&mut test).await?;

    assert_eq!(counts_of(&followee_id).await?.followers, 1);
    Ok(())
}

/// Same for a tag on a stub user: both `tags` and `unique_tags` land.
#[tokio_shared_rt::test(shared)]
async fn test_tag_on_stub_user_is_counted() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let (tagger_id, tagger) = indexed_user(&mut test, "Tagger").await?;
    let (tagged_id, _) = unknown_user(&mut test).await?;
    let ingestor = UserIngestor::default();

    let tag = PubkyAppTag {
        uri: user_uri_builder(tagged_id.clone()),
        label: "stub".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_id = tag.create_id();
    let event_uri = tag_uri_builder(tagger_id, tag_id.clone());

    let first = handlers::tag::sync_put(
        tag.clone(),
        event_uri.clone(),
        tagger.clone(),
        tag_id.clone(),
        &ingestor,
    )
    .await;
    assert!(
        matches!(first, Err(EventProcessorError::MissingDependency { .. })),
        "the first tag should wait for the tagged user: {first:?}"
    );
    handlers::tag::sync_put(tag, event_uri, tagger, tag_id, &ingestor).await?;

    let counts = counts_of(&tagged_id).await?;
    assert_eq!(counts.tags, 1);
    assert_eq!(counts.unique_tags, 1);
    Ok(())
}

/// A stub ingested before stubs got counts has edges but no document. When its profile
/// arrives, the counts are built from the graph rather than written as zeros.
#[tokio_shared_rt::test(shared)]
async fn test_profile_of_legacy_stub_counts_existing_edges() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let (followee_id, followee) = follow_unknown_user(&mut test).await?;
    UserCounts::delete(&followee_id).await?;

    let profile = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Watcher:StubCounts:LateProfile".to_string(),
        status: None,
    };
    handlers::user::sync_put(profile, user_uri_builder(followee_id.clone()), followee).await?;

    assert_eq!(counts_of(&followee_id).await?.followers, 1);
    Ok(())
}
