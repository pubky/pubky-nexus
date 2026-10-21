use super::utils::{assert_user_ingested, create_external_test_homeserver, user_on};
use crate::event_processor::utils::watcher::WatcherTest;
use crate::utils::default_moderation_tests;
use anyhow::Result;
use nexus_common::db::graph::Query;
use nexus_common::db::kv::RedisOps;
use nexus_common::db::{fetch_key_from_graph, fetch_row_from_graph, queries, PubkyConnector};
use nexus_common::models::post::PostDetails;
use nexus_common::models::scope::{HomeserverListScope, Scope};
use nexus_common::models::user::catch_up;
use nexus_common::models::user::stub::create_stub;
use nexus_common::models::user::{
    is_stub, user_hs_cursor_key, PubkyConnectorResolver, UserCounts, UserDetails, UserHsCursor,
    UserIngestor, UserStream, SCOPE_STATE_LEFT, USER_MOSTFOLLOWED_KEY_PARTS,
};
use nexus_common::{ScopeConfig, WatcherConfig};
use nexus_watcher::events::handlers;
use nexus_watcher::events::{DefaultEventHandler, Event, EventHandler, ParseResult};
use nexus_watcher::homeserver::PubkyHomeserverClient;
use nexus_watcher::service::scope_maintenance::ScopeMaintainer;
use pubky::Keypair;
use pubky_app_specs::traits::{HasIdPath, TimestampId};
use pubky_app_specs::{post_uri_builder, PubkyAppPost, PubkyAppPostKind, PubkyAppUser, PubkyId};
use std::path::PathBuf;
use std::sync::Arc;

/// An ingestor whose scope is the test's own homeserver, so users created through
/// the test are in scope and users on any other homeserver are not.
fn scoped_ingestor(test: &WatcherTest) -> UserIngestor {
    let scope: Arc<dyn Scope> = Arc::new(HomeserverListScope::new(std::slice::from_ref(
        &test.homeserver_id,
    )));
    UserIngestor::new(Vec::<PubkyId>::new(), Arc::new(PubkyConnectorResolver)).with_scope(scope)
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
    UserIngestor::new(Vec::<PubkyId>::new(), Arc::new(PubkyConnectorResolver))
        .with_scope(scope)
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

    let client = PubkyHomeserverClient::from_config(
        PubkyConnector::get()?,
        &nexus_common::WatcherConfig::default(),
    );
    let handler = DefaultEventHandler::new(
        default_moderation_tests(),
        ingestor,
        Arc::new(client),
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

    handlers::follow::sync_put(follower_id.clone(), stub_id.clone(), &ingestor).await?;

    let counts = UserCounts::get_from_index(&stub_id)
        .await?
        .expect("stub counts");
    assert_eq!(counts.followers, 1);
    UserStream::refresh_most_followed().await?;
    let rank = |user_id: PubkyId| async move {
        UserDetails::check_sorted_set_member(None, &USER_MOSTFOLLOWED_KEY_PARTS, &[&user_id]).await
    };
    assert!(
        rank(stub_id).await?.is_none(),
        "a stub stays out of most followed"
    );
    assert!(
        rank(follower_id).await?.is_some(),
        "control: a member is ranked"
    );
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

/// The scope's state of `user_id` in the graph: `None` when in scope.
async fn scope_state(user_id: &str) -> Result<Option<String>> {
    let query = queries::get::get_user_scope_state(user_id);
    let state: Option<Option<String>> = fetch_key_from_graph(query, "scope_state").await?;
    Ok(state.flatten())
}

/// A maintainer for a homeserver scope listing `homeservers`, limited to `users`.
fn homeserver_maintainer(
    homeservers: &[PubkyId],
    users: &[&PubkyId],
) -> (UserIngestor, ScopeMaintainer) {
    let mut config = WatcherConfig::default();
    config.stack.scope = ScopeConfig::Homeservers {
        homeservers: homeservers.to_vec(),
    };
    let scope: Arc<dyn Scope> = Arc::new(HomeserverListScope::new(homeservers));
    let ingestor = UserIngestor::new(Vec::<PubkyId>::new(), Arc::new(PubkyConnectorResolver))
        .with_scope(scope);
    let limit_to = Some(users.iter().map(|id| id.to_string()).collect());
    let maintainer = ScopeMaintainer::new(&config, ingestor.clone(), limit_to);
    (ingestor, maintainer)
}

/// A stub whose event comes in from a listed homeserver's feed may have moved there:
/// it is let in, and the event is indexed rather than skipped.
#[tokio_shared_rt::test(shared)]
async fn test_a_stub_that_moved_onto_a_listed_homeserver_is_let_in() -> Result<()> {
    let test = WatcherTest::setup(None).await?;
    let client =
        PubkyHomeserverClient::from_config(PubkyConnector::get()?, &WatcherConfig::default());
    let handler = DefaultEventHandler::new(
        default_moderation_tests(),
        Arc::new(scoped_ingestor(&test)),
        Arc::new(client),
        PathBuf::from("/tmp/nexus-watcher-test"),
    );

    let (user_kp, user_id) = user_on(&test, &test.homeserver_id.to_public_key()).await?;
    create_stub(&UserDetails::from_pubky(user_id.clone())).await?;

    // Written straight to the homeserver, so no event is processed yet.
    let post_id = PubkyAppPost::default().create_id();
    let post_path: pubky::ResourcePath = PubkyAppPost::create_path(&post_id).parse()?;
    let session = PubkyConnector::get()?.signer(user_kp).signin().await?;
    let post = PubkyAppPost {
        content: "Watcher:Scope:MovedIn".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
        lock: None,
    };
    session
        .storage()
        .put(post_path, serde_json::to_string(&post)?)
        .await?;

    let line = format!(
        "PUT {}",
        post_uri_builder(user_id.to_string(), post_id.clone())
    );
    let ParseResult::Parsed(event) = Event::parse_event(&line)? else {
        panic!("a post line parses");
    };
    handler.handle(&event).await?;

    assert!(PostDetails::get_by_id(&user_id, &post_id).await?.is_some());
    assert!(!is_stub(&user_id).await?);
    assert_eq!(scope_state(&user_id).await?, None, "in scope");
    Ok(())
}

/// Narrowing a homeserver list lets the users on the dropped homeserver go; listing it
/// again brings them back, although they left rather than being stubs.
#[tokio_shared_rt::test(shared)]
async fn test_listing_a_homeserver_again_brings_its_users_back() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (_, user_id) = user_on(&test, &hs_pk).await?;
    let both = [test.homeserver_id.clone(), PubkyId::from(hs_pk)];

    let (ingestor, wide) = homeserver_maintainer(&both, &[&user_id]);
    ingestor.maybe_ingest_user(&user_id).await?;
    assert_eq!(scope_state(&user_id).await?, None, "in scope");

    let (_, narrow) = homeserver_maintainer(&both[..1], &[&user_id]);
    let narrowed = narrow.at_start().await?;
    assert_eq!(narrowed.left, 1);
    assert_eq!(
        scope_state(&user_id).await?.as_deref(),
        Some(SCOPE_STATE_LEFT)
    );

    let widened = wide.at_start().await?;
    assert_eq!(widened.joined, 1);
    assert_eq!(scope_state(&user_id).await?, None, "back in scope");
    Ok(())
}

/// Switching to "everything" lets every stub in.
#[tokio_shared_rt::test(shared)]
async fn test_everything_lets_stubs_back_in() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (_, user_id) = user_on(&test, &hs_pk).await?;
    let hs_id = PubkyId::from(hs_pk).to_string();
    scoped_ingestor(&test).maybe_ingest_user(&user_id).await?;
    assert_user_is_stub(&user_id).await;

    let maintainer = ScopeMaintainer::new(
        &WatcherConfig::default(),
        UserIngestor::new([], Arc::new(PubkyConnectorResolver)),
        Some(vec![user_id.to_string()]),
    );
    let change = maintainer.at_start().await?;
    let catching_up = catch_up::catching_up(&[user_id.as_ref()]).await?;
    catch_up::caught_up(&user_id).await?;

    assert_eq!(change.joined, 1);
    assert!(!is_stub(&user_id).await?);
    assert_eq!(scope_state(&user_id).await?, None, "in scope");
    assert_eq!(
        catching_up,
        vec![true],
        "its skipped records are read from the start"
    );
    let cursors = UserHsCursor::read(&[user_id.as_ref()], &hs_id).await?;
    assert_eq!(cursors, vec![0]);
    Ok(())
}

/// A homeserver scope that couldn't find a user's homeserver at start (not published
/// yet, or the lookup timed out) tries again each interval and admits it once found.
#[tokio_shared_rt::test(shared)]
async fn test_an_admission_whose_homeserver_was_not_found_is_retried() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let late_kp = Keypair::random();
    let late = PubkyId::from(late_kp.public_key());
    create_stub(&UserDetails::from_pubky(late.clone())).await?;

    let homeservers = [test.homeserver_id.clone(), PubkyId::from(hs_pk.clone())];
    let (_, maintainer) = homeserver_maintainer(&homeservers, &[&late]);
    let change = maintainer.at_start().await?;
    assert_eq!(change.joined, 0, "no homeserver found yet");

    test.register_user_in_hs(&late_kp, &hs_pk).await?;
    let change = maintainer.tick().await?;

    assert_eq!(change.joined, 1, "found and admitted on the next interval");
    assert!(!is_stub(&late).await?);
    Ok(())
}

/// A user let back in is read from the start, even if a poll still running when it left
/// wrote a cursor after its cursors were removed.
#[tokio_shared_rt::test(shared)]
async fn test_a_user_let_back_in_is_read_from_the_start() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (_, user_id) = user_on(&test, &hs_pk).await?;
    let hs_id = PubkyId::from(hs_pk).to_string();
    let ingestor = UserIngestor::new([], Arc::new(PubkyConnectorResolver));
    ingestor.maybe_ingest_user(&user_id).await?;
    ingestor.demote(&user_id).await?;
    // What a poll still running when the user left writes back.
    UserHsCursor::write(&user_id, &hs_id, 500).await?;

    assert!(ingestor.promote(&user_id).await?);

    let cursors = UserHsCursor::read(&[user_id.as_ref()], &hs_id).await?;
    assert_eq!(cursors, vec![0], "read from the start");
    Ok(())
}

/// An unknown author whose homeserver isn't listed is placed by the homeserver it
/// publishes, not by the listed feed its old event came from: the event is skipped and
/// the author kept as a stub.
#[tokio_shared_rt::test(shared)]
async fn test_an_unknown_author_that_moved_away_is_not_indexed() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (_, author_id) = user_on(&test, &hs_pk).await?;
    let client = PubkyHomeserverClient::from_config(
        PubkyConnector::get()?,
        &nexus_common::WatcherConfig::default(),
    );
    let handler = DefaultEventHandler::new(
        default_moderation_tests(),
        Arc::new(scoped_ingestor(&test)),
        Arc::new(client),
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
    assert_user_is_stub(&author_id).await;
    Ok(())
}
