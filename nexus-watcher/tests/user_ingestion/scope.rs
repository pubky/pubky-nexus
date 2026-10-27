use super::utils::{assert_user_ingested, create_external_test_homeserver, user_on};
use crate::event_processor::utils::watcher::WatcherTest;
use crate::utils::default_moderation_tests;
use anyhow::Result;
use nexus_common::db::graph::Query;
use nexus_common::db::kv::RedisOps;
use nexus_common::db::{fetch_key_from_graph, fetch_row_from_graph, queries, PubkyConnector};
use nexus_common::models::homeserver::Homeserver;
use nexus_common::models::post::PostDetails;
use nexus_common::models::scope::{HomeserverListScope, Scope};
use nexus_common::models::user::catch_up;
use nexus_common::models::user::outsider::{admit_outsider, create_outsider};
use nexus_common::models::user::TAG_GLOBAL_USER_TAGGERS;
use nexus_common::models::user::{
    is_outsider, user_hs_cursor_key, PubkyConnectorResolver, UserCounts, UserDetails, UserHsCursor,
    UserIngestor, UserStream, SCOPE_STATE_LEFT, USER_MOSTFOLLOWED_KEY_PARTS,
};
use nexus_common::{ScopeConfig, WatcherConfig};
use nexus_watcher::events::handlers;
use nexus_watcher::events::{DefaultEventHandler, Event, EventHandler, ParseResult};
use nexus_watcher::homeserver::PubkyHomeserverClient;
use nexus_watcher::service::scope_maintenance::ScopeMaintainer;
use nexus_watcher::service::{HsEventProcessor, TEventProcessor};
use pubky::Keypair;
use pubky_app_specs::traits::{HasIdPath, HashId, TimestampId};
use pubky_app_specs::{
    post_uri_builder, tag_uri_builder, user_uri_builder, PubkyAppPost, PubkyAppPostKind,
    PubkyAppTag, PubkyAppUser, PubkyId,
};
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

/// The processor of the test homeserver's whole feed, deciding with `ingestor`'s scope.
fn feed_processor(test: &WatcherTest, ingestor: Arc<UserIngestor>) -> HsEventProcessor {
    let runner = &test.event_processor_runner;
    HsEventProcessor {
        homeserver: Homeserver::new(test.homeserver_id.clone()),
        limit: runner.limit,
        event_handler: runner.event_handler.clone(),
        client: runner.client.clone(),
        shutdown_rx: runner.shutdown_rx.clone(),
        retry_scheduler: runner.retry_scheduler.clone(),
        catch_up_resume_points: Default::default(),
        catch_up_not_found: Default::default(),
        ingestor,
        hs_mapping_cache: Default::default(),
    }
}

/// A `PUT` event of a post by `author_id` that was never written to any homeserver.
fn unwritten_post_event(author_id: &PubkyId) -> Result<(Event, String)> {
    let post_id = PubkyAppPost::default().create_id();
    let line = format!(
        "PUT {}",
        post_uri_builder(author_id.to_string(), post_id.clone())
    );
    let ParseResult::Parsed(event) = Event::parse_event(&line)? else {
        panic!("a post line parses");
    };
    Ok((event, post_id))
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

/// Asserts the user is known but kept outside the scope: a node marked as an outsider,
/// with no `HOSTED_BY` edge and no cursor, so nothing ever polls it.
async fn assert_user_is_outsider(user_id: &str) {
    assert!(
        UserDetails::get_by_id(user_id).await.unwrap().is_some(),
        "the outsider's node exists, so records can point at it"
    );
    assert!(
        is_outsider(user_id).await.unwrap(),
        "{user_id} should be an outsider"
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
    assert!(cursors.is_none(), "an outsider has no cursor");

    let query = Query::new(
        "assert_outsider_not_hosted",
        "MATCH (u:User {id: $user_id})-[:HOSTED_BY]->(hs:Homeserver) RETURN hs.id AS hs_id",
    )
    .param("user_id", user_id.to_string());
    let hosted_by = fetch_row_from_graph(query).await.unwrap();
    assert!(
        hosted_by.is_none(),
        "an outsider is not bound to a homeserver"
    );
}

/// A user on a homeserver outside the list is kept as an outsider, and the HS resolver
/// never picks it up.
#[tokio_shared_rt::test(shared)]
async fn test_user_outside_the_scope_is_kept_as_a_outsider() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (_, user_id) = user_on(&test, &hs_pk).await?;

    scoped_ingestor(&test).maybe_ingest_user(&user_id).await?;

    assert_user_is_outsider(&user_id).await;

    let needing_resolution: Vec<String> =
        fetch_key_from_graph(queries::get::get_users_needing_hs_resolution(0), "user_ids")
            .await?
            .unwrap_or_default();
    assert!(
        !needing_resolution.contains(&user_id.to_string()),
        "the resolver skips outsiders, so they never get HOSTED_BY"
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
    assert!(!is_outsider(&user_id).await?);
    Ok(())
}

/// A whole feed holds everyone on its homeserver: an event of an author outside the
/// scope read from one is skipped before it is fetched. Handed to the handler as the retry
/// queue hands it, the same event is processed: a queued event is finished whatever the
/// scope.
#[tokio_shared_rt::test(shared)]
async fn test_event_by_an_author_outside_the_scope_is_skipped() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (_, author_id) = user_on(&test, &hs_pk).await?;
    let ingestor = Arc::new(scoped_ingestor(&test));
    ingestor.maybe_ingest_user(&author_id).await?;
    assert_user_is_outsider(&author_id).await;
    let (event, post_id) = unwritten_post_event(&author_id)?;

    let from_the_feed = feed_processor(&test, ingestor.clone())
        .should_process_event(&event)
        .await?;
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
    let retried = handler.handle(&event).await;

    assert!(!from_the_feed, "skipped when read from the feed");
    assert!(
        retried.is_err(),
        "processed when retried: the fetch of the unwritten post fails"
    );
    assert!(PostDetails::get_by_id(&author_id, &post_id)
        .await?
        .is_none());
    Ok(())
}

/// Members following an outsider move its counts, but it stays out of the rankings,
/// where it would show as a blank card.
#[tokio_shared_rt::test(shared)]
async fn test_a_followed_outsider_stays_out_of_the_rankings() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (_, outsider_id) = user_on(&test, &hs_pk).await?;
    let follower_id = member(&mut test, "Follower").await?;
    let ingestor = scoped_ingestor(&test);
    ingestor.maybe_ingest_user(&outsider_id).await?;

    handlers::follow::sync_put(follower_id.clone(), outsider_id.clone(), &ingestor).await?;

    let counts = UserCounts::get_by_id(&outsider_id)
        .await?
        .expect("outsider counts");
    assert_eq!(counts.followers, 1);
    UserStream::refresh_most_followed().await?;
    let rank = |user_id: PubkyId| async move {
        UserDetails::check_sorted_set_member(None, &USER_MOSTFOLLOWED_KEY_PARTS, &[&user_id]).await
    };
    assert!(
        rank(outsider_id).await?.is_none(),
        "an outsider stays out of most followed"
    );
    assert!(
        rank(follower_id).await?.is_some(),
        "control: a member is ranked"
    );
    Ok(())
}

/// A user that must always be read (the moderator), kept as an outsider elsewhere (the API
/// ingests with the plain scope), is brought back into the scope and bound at start.
#[tokio_shared_rt::test(shared)]
async fn test_ensure_in_scope_brings_a_moderator_kept_outside_back() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (_, moderator_id) = user_on(&test, &hs_pk).await?;
    let ingestor = scoped_ingestor(&test);
    ingestor.maybe_ingest_user(&moderator_id).await?;
    assert_user_is_outsider(&moderator_id).await;

    ingestor.ensure_in_scope(&moderator_id).await?;

    assert!(!is_outsider(&moderator_id).await?);
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
    homeserver_maintainer_moderated_by(homeservers, users, None)
}

/// [`homeserver_maintainer`] with `moderator` as the moderator, if given.
fn homeserver_maintainer_moderated_by(
    homeservers: &[PubkyId],
    users: &[&PubkyId],
    moderator: Option<&PubkyId>,
) -> (UserIngestor, ScopeMaintainer) {
    let mut config = WatcherConfig::default();
    if let Some(moderator) = moderator {
        config.moderation_id = moderator.clone();
    }
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

/// Whether `user_id` has a `HOSTED_BY` edge to `hs_id`, and a cursor there.
async fn binding(user_id: &str, hs_id: &str) -> Result<(bool, bool)> {
    let query = Query::new(
        "test_user_hosted_by",
        "MATCH (u:User {id: $user_id})-[:HOSTED_BY]->(hs:Homeserver {id: $hs_id})
         RETURN hs.id AS homeserver_id",
    )
    .param("user_id", user_id.to_string())
    .param("hs_id", hs_id.to_string());
    let hosted_by = fetch_row_from_graph(query).await?.is_some();
    let cursor = UserDetails::check_sorted_set_member(None, &user_hs_cursor_key(user_id), &[hs_id])
        .await?
        .is_some();
    Ok((hosted_by, cursor))
}

/// The moderator, bound to a homeserver the list doesn't hold, is never let go: neither
/// at start, right after it was let in, nor at a tick. Its takedowns must always be read.
#[tokio_shared_rt::test(shared)]
async fn test_the_moderator_is_never_let_go() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (_, moderator) = user_on(&test, &hs_pk).await?;
    let hs_id = PubkyId::from(hs_pk).to_string();
    // Bound to its unlisted homeserver, with a cursor there.
    UserIngestor::new(Vec::<PubkyId>::new(), Arc::new(PubkyConnectorResolver))
        .maybe_ingest_user(&moderator)
        .await?;
    let listed = [test.homeserver_id.clone()];
    let (_, maintainer) =
        homeserver_maintainer_moderated_by(&listed, &[&moderator], Some(&moderator));

    let at_start = maintainer.at_start().await?;
    let ticked = maintainer.tick().await?;

    assert_eq!(at_start.left, 0, "kept at start");
    assert_eq!(ticked.left, 0, "kept at a tick");
    assert_eq!(scope_state(&moderator).await?, None, "in scope");
    assert_eq!(
        binding(&moderator, &hs_id).await?,
        (true, true),
        "still bound, with its cursor"
    );
    Ok(())
}

/// An outsider whose event comes in from a listed homeserver's feed may have moved there:
/// the feed's processor lets it in, and the event is indexed rather than skipped.
#[tokio_shared_rt::test(shared)]
async fn test_an_outsider_that_moved_onto_a_listed_homeserver_is_let_in() -> Result<()> {
    let test = WatcherTest::setup(None).await?;
    let processor = feed_processor(&test, Arc::new(scoped_ingestor(&test)));

    let (user_kp, user_id) = user_on(&test, &test.homeserver_id.to_public_key()).await?;
    create_outsider(&UserDetails::from_pubky(user_id.clone())).await?;

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
    assert!(
        processor.should_process_event(&event).await?,
        "let in, so its event is processed"
    );
    processor.event_handler.handle(&event).await?;

    assert!(PostDetails::get_by_id(&user_id, &post_id).await?.is_some());
    assert!(!is_outsider(&user_id).await?);
    assert_eq!(scope_state(&user_id).await?, None, "in scope");
    Ok(())
}

/// Narrowing a homeserver list lets the users on the dropped homeserver go; listing it
/// again brings them back, although they left rather than being outsiders.
#[tokio_shared_rt::test(shared)]
async fn test_listing_a_homeserver_again_brings_its_users_back() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (_, user_id) = user_on(&test, &hs_pk).await?;
    let both = [test.homeserver_id.clone(), PubkyId::from(hs_pk)];

    let (ingestor, wide) = homeserver_maintainer(&both, &[&user_id]);
    ingestor.maybe_ingest_user(&user_id).await?;
    assert_eq!(scope_state(&user_id).await?, None, "in scope");
    // Still catching up when the list narrows.
    let hs_id = both[1].to_string();
    catch_up::start_catch_up(&user_id, &hs_id).await?;

    let (_, narrow) = homeserver_maintainer(&both[..1], &[&user_id]);
    let narrowed = narrow.at_start().await?;
    assert_eq!(narrowed.left, 1);
    assert_eq!(
        scope_state(&user_id).await?.as_deref(),
        Some(SCOPE_STATE_LEFT)
    );
    // Unbound, so a whole-feed processor hands its later events to `admit_if_moved_in`
    // rather than skipping them as hosted elsewhere.
    assert_eq!(
        binding(&user_id, &hs_id).await?,
        (false, false),
        "no HOSTED_BY, no cursor"
    );
    assert_eq!(
        catch_up::catching_up(&[user_id.as_ref()]).await?,
        vec![false],
        "no catch-up entry"
    );

    let widened = wide.at_start().await?;
    assert_eq!(widened.joined, 1);
    assert_eq!(scope_state(&user_id).await?, None, "back in scope");
    Ok(())
}

/// Switching to "everything" lets every outsider in.
#[tokio_shared_rt::test(shared)]
async fn test_everything_lets_outsiders_back_in() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (_, user_id) = user_on(&test, &hs_pk).await?;
    let hs_id = PubkyId::from(hs_pk).to_string();
    scoped_ingestor(&test).maybe_ingest_user(&user_id).await?;
    assert_user_is_outsider(&user_id).await;

    let maintainer = ScopeMaintainer::new(
        &WatcherConfig::default(),
        UserIngestor::new([], Arc::new(PubkyConnectorResolver)),
        Some(vec![user_id.to_string()]),
    );
    let change = maintainer.at_start().await?;
    let catching_up = catch_up::catching_up(&[user_id.as_ref()]).await?;
    catch_up::caught_up(&user_id).await?;

    assert_eq!(change.joined, 1);
    assert!(!is_outsider(&user_id).await?);
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
    create_outsider(&UserDetails::from_pubky(late.clone())).await?;

    let homeservers = [test.homeserver_id.clone(), PubkyId::from(hs_pk.clone())];
    let (_, maintainer) = homeserver_maintainer(&homeservers, &[&late]);
    let change = maintainer.at_start().await?;
    assert_eq!(change.joined, 0, "no homeserver found yet");

    test.register_user_in_hs(&late_kp, &hs_pk).await?;
    let change = maintainer.tick().await?;

    assert_eq!(change.joined, 1, "found and admitted on the next interval");
    assert!(!is_outsider(&late).await?);
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
/// the author kept as an outsider.
#[tokio_shared_rt::test(shared)]
async fn test_an_unknown_author_that_moved_away_is_not_indexed() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (_, author_id) = user_on(&test, &hs_pk).await?;
    let (event, _) = unwritten_post_event(&author_id)?;

    let processor = feed_processor(&test, Arc::new(scoped_ingestor(&test)));
    let processed = processor.should_process_event(&event).await?;

    assert!(!processed, "skipped before it is fetched");
    assert_user_is_outsider(&author_id).await;
    Ok(())
}

/// A member's tag on an outsider indexes the tag, but the outsider stays out of the
/// users-by-tag search, where it would show as a blank card; a member tagged the same
/// way is in. Once the outsider joins the scope, it is ranked there.
#[tokio_shared_rt::test(shared)]
async fn test_a_tagged_outsider_stays_out_of_the_users_by_tag_search() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (_, outsider_id) = user_on(&test, &hs_pk).await?;
    let member_id = member(&mut test, "Tagged").await?;
    let tagger_id = member(&mut test, "Tagger").await?;
    let ingestor = scoped_ingestor(&test);
    let label = format!("scopetest{}", chrono::Utc::now().timestamp_micros());

    for tagged in [&outsider_id, &member_id] {
        let tag = PubkyAppTag {
            uri: user_uri_builder(tagged.to_string()),
            label: label.clone(),
            created_at: chrono::Utc::now().timestamp_millis(),
        };
        let tag_id = tag.create_id();
        let tag_uri = tag_uri_builder(tagger_id.to_string(), tag_id.clone());
        // The first meets an unknown user: it is ingested, and the tag retried.
        for _ in 0..2 {
            let put = handlers::tag::sync_put(
                tag.clone(),
                tag_uri.clone(),
                tagger_id.clone(),
                tag_id.clone(),
                &ingestor,
            )
            .await;
            if put.is_ok() {
                break;
            }
        }
    }
    assert_user_is_outsider(&outsider_id).await;

    let key_parts = [&TAG_GLOBAL_USER_TAGGERS[..], &[label.as_str()]].concat();
    let ranked = |user_id: PubkyId| {
        let key_parts = key_parts.clone();
        async move { UserDetails::check_sorted_set_member(None, &key_parts, &[&user_id]).await }
    };
    let outsider_while_out = ranked(outsider_id.clone()).await?;
    let member_ranked = ranked(member_id).await?;
    admit_outsider(&outsider_id).await?;
    let outsider_once_in = ranked(outsider_id.clone()).await?;

    assert!(
        outsider_while_out.is_none(),
        "an outsider stays out of the tag search"
    );
    assert!(member_ranked.is_some(), "control: a member is in");
    assert!(outsider_once_in.is_some(), "ranked once it joins the scope");
    Ok(())
}
