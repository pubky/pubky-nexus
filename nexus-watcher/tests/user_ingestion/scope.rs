use super::utils::{assert_user_ingested, create_external_test_homeserver, user_on};
use crate::event_processor::utils::watcher::WatcherTest;
use crate::utils::default_moderation_tests;
use anyhow::Result;
use nexus_common::db::graph::Query;
use nexus_common::db::kv::RedisOps;
use nexus_common::db::{fetch_key_from_graph, fetch_row_from_graph, queries};
use nexus_common::models::post::PostDetails;
use nexus_common::models::scope::{HomeserverListScope, Scope};
use nexus_common::models::user::{
    is_stub, user_hs_cursor_key, UserCounts, UserDetails, UserIngestor, USER_MOSTFOLLOWED_KEY_PARTS,
};
use nexus_common::DEFAULT_MAX_FILE_SIZE;
use nexus_watcher::events::handlers;
use nexus_watcher::events::{DefaultEventHandler, Event, EventHandler, ParseResult};
use pubky::Keypair;
use pubky_app_specs::traits::TimestampId;
use pubky_app_specs::{post_uri_builder, PubkyAppPost, PubkyAppUser, PubkyId};
use std::path::PathBuf;
use std::sync::Arc;

/// An ingestor whose scope is the test's own homeserver, so users created through
/// the test are in scope and users on any other homeserver are not.
fn scoped_ingestor(test: &WatcherTest) -> UserIngestor {
    let scope: Arc<dyn Scope> = Arc::new(HomeserverListScope::new(std::slice::from_ref(
        &test.homeserver_id,
    )));
    UserIngestor::with_scope(Vec::<PubkyId>::new(), scope)
}

/// A user created on the test's homeserver, so in scope.
async fn member(test: &mut WatcherTest, name: &str) -> Result<PubkyId> {
    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: format!("Watcher:Scope:{name}"),
        status: None,
    };
    test.create_user(&user_kp, &user).await?;
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
    let (_, user_id) = user_on(&test, &hs_pk).await?;

    scoped_ingestor(&test).maybe_ingest_user(&user_id).await?;

    assert_user_is_stub(&user_id).await;
    assert!(
        UserCounts::get_from_index(&user_id).await?.is_some(),
        "a stub has counts, so records that move them don't fail"
    );

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

/// Control: a user on a listed homeserver is ingested as before.
#[tokio_shared_rt::test(shared)]
async fn test_user_inside_the_scope_is_ingested_as_before() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (_, user_id) = user_on(&test, &hs_pk).await?;

    let scope: Arc<dyn Scope> = Arc::new(HomeserverListScope::new(&[PubkyId::from(hs_pk.clone())]));
    UserIngestor::with_scope(Vec::<PubkyId>::new(), scope)
        .maybe_ingest_user(&user_id)
        .await?;

    assert_user_ingested(&user_id, &hs_pk).await;
    assert!(!is_stub(&user_id).await?);
    Ok(())
}

/// An event by an author outside the scope is skipped before it is fetched: nothing is
/// indexed and the event doesn't fail.
#[tokio_shared_rt::test(shared)]
async fn test_event_by_an_author_outside_the_scope_is_skipped() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (_, author_id) = user_on(&test, &hs_pk).await?;
    let ingestor = Arc::new(scoped_ingestor(&test));
    ingestor.maybe_ingest_user(&author_id).await?;
    assert_user_is_stub(&author_id).await;

    let handler = DefaultEventHandler::new(
        default_moderation_tests(),
        ingestor,
        DEFAULT_MAX_FILE_SIZE,
        PathBuf::from("/tmp/nexus-watcher-test"),
    );
    // Never written to any homeserver: fetching it would fail, so `Ok` means skipped.
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

/// Members following a stub move its counts, but it stays out of the rankings,
/// where it would show as a blank card.
#[tokio_shared_rt::test(shared)]
async fn test_a_followed_stub_stays_out_of_the_rankings() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (_, stub_id) = user_on(&test, &hs_pk).await?;
    let follower_id = member(&mut test, "Follower").await?;
    let ingestor = scoped_ingestor(&test);
    ingestor.maybe_ingest_user(&stub_id).await?;

    handlers::follow::sync_put(follower_id, stub_id.clone(), &ingestor).await?;

    let counts = UserCounts::get_from_index(&stub_id)
        .await?
        .expect("stub counts");
    assert_eq!(counts.followers, 1);
    let ranked =
        UserDetails::check_sorted_set_member(None, &USER_MOSTFOLLOWED_KEY_PARTS, &[&stub_id])
            .await?;
    assert!(ranked.is_none(), "a stub stays out of most followed");
    Ok(())
}

/// A user that must always be read (the moderator), kept as a stub elsewhere (the API
/// ingests with the plain scope), is brought back into the scope and bound at start.
#[tokio_shared_rt::test(shared)]
async fn test_ensure_in_scope_brings_a_stubbed_moderator_back() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (_, moderator_id) = user_on(&test, &hs_pk).await?;
    let ingestor = scoped_ingestor(&test);
    ingestor.maybe_ingest_user(&moderator_id).await?;
    assert_user_is_stub(&moderator_id).await;

    ingestor.ensure_in_scope(&moderator_id).await?;

    assert!(!is_stub(&moderator_id).await?);
    assert_user_ingested(&moderator_id, &hs_pk).await;
    let query = queries::get::get_user_scope_state(&moderator_id);
    let scope_state: Option<Option<String>> = fetch_key_from_graph(query, "scope_state").await?;
    assert_eq!(scope_state, Some(None), "no scope_state left in the graph");
    Ok(())
}
