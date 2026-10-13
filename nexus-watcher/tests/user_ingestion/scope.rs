use super::utils::{assert_user_ingested, create_external_test_homeserver};
use crate::event_processor::utils::watcher::WatcherTest;
use crate::utils::default_moderation_tests;
use anyhow::Result;
use nexus_common::db::graph::Query;
use nexus_common::db::kv::RedisOps;
use nexus_common::db::{fetch_key_from_graph, fetch_row_from_graph, queries};
use nexus_common::models::post::PostDetails;
use nexus_common::models::scope::{HomeserverListScope, Scope, SeedHopsScope};
use nexus_common::models::user::{is_stub, user_hs_cursor_key, UserDetails, UserIngestor};
use nexus_common::utils::test_utils::random_pubky_id;
use nexus_common::DEFAULT_MAX_FILE_SIZE;
use nexus_watcher::events::handlers;
use nexus_watcher::events::{DefaultEventHandler, Event, EventHandler, ParseResult};
use nexus_watcher::EventProcessorError;
use pubky::Keypair;
use pubky_app_specs::traits::TimestampId;
use pubky_app_specs::{post_uri_builder, PubkyAppPost, PubkyAppPostKind, PubkyAppUser, PubkyId};
use std::path::PathBuf;
use std::sync::Arc;

fn scoped_ingestor(scope: Arc<dyn Scope>) -> UserIngestor {
    UserIngestor::with_scope(Vec::<PubkyId>::new(), scope)
}

/// Registers a fresh user on `hs_pk` and returns its id.
async fn user_on(test: &WatcherTest, hs_pk: &pubky::PublicKey) -> Result<PubkyId> {
    let user_kp = Keypair::random();
    test.register_user_in_hs(&user_kp, hs_pk).await?;
    Ok(PubkyId::from(user_kp.public_key()))
}

/// Asserts the user is known but kept outside the scope: a node marked as a stub,
/// with no `HOSTED_BY` edge and no cursor, so nothing ever polls it.
async fn assert_user_is_stub(user_id: &str) {
    assert!(
        UserDetails::get_by_id(user_id).await.unwrap().is_some(),
        "the stub's node exists, so records can point at it"
    );
    assert!(
        is_stub(user_id).await.unwrap(),
        "{user_id} should be a stub"
    );

    let key = user_hs_cursor_key(user_id);
    let cursors = UserDetails::try_from_index_sorted_set(
        &key,
        None,
        None,
        None,
        None,
        Default::default(),
        None,
    )
    .await
    .unwrap();
    assert!(cursors.is_none(), "a stub has no cursor");

    let query = Query::new(
        "assert_stub_not_hosted",
        "MATCH (u:User {id: $user_id})-[:HOSTED_BY]->(hs:Homeserver) RETURN hs.id AS hs_id",
    )
    .param("user_id", user_id.to_string());
    let hosted_by = fetch_row_from_graph(query).await.unwrap();
    assert!(hosted_by.is_none(), "a stub is not bound to a homeserver");
}

/// A user on a homeserver outside the list is kept as a stub, and the HS resolver
/// never picks it up.
#[tokio_shared_rt::test(shared)]
async fn test_user_outside_the_scope_is_kept_as_a_stub() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let user_id = user_on(&test, &hs_pk).await?;

    let ingestor = scoped_ingestor(Arc::new(HomeserverListScope::new(&[random_pubky_id()])));
    ingestor.maybe_ingest_user(&user_id).await?;

    assert_user_is_stub(&user_id).await;

    let needing_resolution: Vec<String> =
        fetch_key_from_graph(queries::get::get_users_needing_hs_resolution(0), "user_ids")
            .await?
            .unwrap_or_default();
    assert!(
        !needing_resolution.contains(&user_id.to_string()),
        "the resolver skips stubs, so they never get HOSTED_BY"
    );
    Ok(())
}

/// Control: with the user's homeserver listed, the user is ingested as before.
#[tokio_shared_rt::test(shared)]
async fn test_user_inside_the_scope_is_ingested_as_before() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let user_id = user_on(&test, &hs_pk).await?;

    let listed = PubkyId::from(hs_pk.clone());
    let ingestor = scoped_ingestor(Arc::new(HomeserverListScope::new(&[listed])));
    ingestor.maybe_ingest_user(&user_id).await?;

    assert_user_ingested(&user_id, &hs_pk).await;
    assert!(!is_stub(&user_id).await?);
    Ok(())
}

/// The seed's follow brings a stub into the scope: it is bound to its homeserver and
/// polled from then on. The followee's own follows stop at the hop limit.
#[tokio_shared_rt::test(shared)]
async fn test_seed_follow_promotes_a_stub_and_stops_at_the_hop_limit() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;

    let seed_kp = Keypair::random();
    let seed_user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Watcher:Scope:Seed".to_string(),
        status: None,
    };
    test.create_user(&seed_kp, &seed_user).await?;
    let seed_id = PubkyId::from(seed_kp.public_key());

    let scope = Arc::new(SeedHopsScope::new(seed_id.clone(), 1));
    scope.prepare().await?;
    let ingestor = scoped_ingestor(scope.clone());

    // Discovered before anyone in scope follows it: kept as a stub.
    let followee_id = user_on(&test, &hs_pk).await?;
    ingestor.maybe_ingest_user(&followee_id).await?;
    assert_user_is_stub(&followee_id).await;

    handlers::follow::sync_put(seed_id.clone(), followee_id.clone(), &ingestor).await?;

    assert_eq!(scope.hop_of(&followee_id).await?, Some(1));
    assert!(!is_stub(&followee_id).await?, "promoted out of stub");
    assert_user_ingested(&followee_id, &hs_pk).await;

    // Hop 1 is the limit: who the followee follows stays outside the scope.
    let beyond_id = user_on(&test, &hs_pk).await?;
    let result = handlers::follow::sync_put(followee_id, beyond_id.clone(), &ingestor).await;
    assert!(
        matches!(result, Err(EventProcessorError::MissingDependency { .. })),
        "the unknown followee is ingested and the follow retried, as today: {result:?}"
    );
    assert_user_is_stub(&beyond_id).await;
    assert_eq!(scope.hop_of(&beyond_id).await?, None);
    Ok(())
}

/// The handler skips an event whose author is outside the scope, before fetching it:
/// nothing is indexed and the event doesn't fail.
#[tokio_shared_rt::test(shared)]
async fn test_event_by_an_author_outside_the_scope_is_skipped() -> Result<()> {
    WatcherTest::setup(None).await?;

    let scope = Arc::new(SeedHopsScope::new(random_pubky_id(), 2));
    scope.prepare().await?;
    let handler = DefaultEventHandler::new(
        default_moderation_tests(),
        Arc::new(scoped_ingestor(scope)),
        DEFAULT_MAX_FILE_SIZE,
        PathBuf::from("/tmp/nexus-watcher-test"),
    );

    // Never written to any homeserver: fetching it would fail, so `Ok` means skipped.
    let author_id = random_pubky_id();
    let post_id = PubkyAppPost::default().create_id();
    let line = format!(
        "PUT {}",
        post_uri_builder(author_id.to_string(), post_id.clone())
    );
    let ParseResult::Parsed(event) = Event::parse_event(&line)? else {
        panic!("a post line parses");
    };

    handler.handle(&event).await?;

    assert!(PostDetails::get_by_id(&author_id, &post_id)
        .await?
        .is_none());
    Ok(())
}

/// A reply by an in-scope user to a post by a stub lands at once: that one post is
/// fetched and indexed, its author stays a stub, and its other posts are not read.
#[tokio_shared_rt::test(shared)]
async fn test_reply_to_a_post_outside_the_scope_fetches_only_that_post() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;

    let outsider_kp = Keypair::random();
    test.register_user_in_hs(&outsider_kp, &hs_pk).await?;
    let outsider_id = PubkyId::from(outsider_kp.public_key());
    let post = |content: &str| PubkyAppPost {
        content: format!("Watcher:Scope:Outsider:{content}"),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
        lock: None,
    };
    // Written to the outsider's homeserver, which the test never indexes.
    let (parent_id, _) = test.create_post(&outsider_kp, &post("Parent")).await?;
    let (other_id, _) = test.create_post(&outsider_kp, &post("Other")).await?;

    let seed_kp = Keypair::random();
    let seed_user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Watcher:Scope:Replier".to_string(),
        status: None,
    };
    test.create_user(&seed_kp, &seed_user).await?;
    let seed_id = PubkyId::from(seed_kp.public_key());
    let scope = Arc::new(SeedHopsScope::new(seed_id.clone(), 1));
    scope.prepare().await?;
    let ingestor = scoped_ingestor(scope);

    ingestor.maybe_ingest_user(&outsider_id).await?;
    assert_user_is_stub(&outsider_id).await;

    let reply = PubkyAppPost {
        parent: Some(post_uri_builder(outsider_id.to_string(), parent_id.clone())),
        ..post("Reply")
    };
    let reply_id = reply.create_id();
    let reply_uri = post_uri_builder(seed_id.to_string(), reply_id.clone());
    handlers::post::sync_put(
        reply,
        reply_uri,
        seed_id.clone(),
        reply_id.clone(),
        &ingestor,
    )
    .await?;

    assert!(
        PostDetails::get_by_id(&seed_id, &reply_id).await?.is_some(),
        "the reply lands"
    );
    assert!(
        PostDetails::get_by_id(&outsider_id, &parent_id)
            .await?
            .is_some(),
        "its parent is fetched"
    );
    assert!(
        PostDetails::get_by_id(&outsider_id, &other_id)
            .await?
            .is_none(),
        "nothing else of the outsider's is read"
    );
    assert_user_is_stub(&outsider_id).await;
    Ok(())
}
