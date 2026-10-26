use super::utils::{assert_user_ingested, create_external_test_homeserver, user_on};
use crate::event_processor::utils::watcher::{rank_user, unrank_user, WatcherTest};
use crate::utils::default_moderation_tests;
use anyhow::Result;
use deadpool_redis::redis::AsyncCommands;
use nexus_common::db::graph::Query;
use nexus_common::db::kv::RedisOps;
use nexus_common::db::{
    fetch_key_from_graph, fetch_row_from_graph, get_redis_conn, queries, PubkyConnector,
};
use nexus_common::models::homeserver::Homeserver;
use nexus_common::models::post::{PostDetails, PostRelationships, POST_TIMELINE_KEY_PARTS};
use nexus_common::models::scope::{HomeserverListScope, Scope};
use nexus_common::models::tag::user::USER_TAGS_KEY_PARTS;
use nexus_common::models::takedown::record_takedown;
use nexus_common::models::user::catch_up;
use nexus_common::models::user::outsider::{admit_outsider, create_outsider};
use nexus_common::models::user::TAG_GLOBAL_USER_TAGGERS;
use nexus_common::models::user::{
    is_outsider, user_hs_cursor_key, PkdnsHomeserverResolver, PubkyConnectorResolver, UserCounts,
    UserDetails, UserHsCursor, UserIngestor, UserStream, SCOPE_STATE_LEFT,
    USER_MOSTFOLLOWED_KEY_PARTS,
};
use nexus_common::{ScopeConfig, WatcherConfig};
use nexus_watcher::events::handlers;
use nexus_watcher::events::{DefaultEventHandler, Event, EventHandler, ParseResult};
use nexus_watcher::homeserver::{HomeserverClient, HsError, HsStreamEvent, PubkyHomeserverClient};
use nexus_watcher::service::scope_maintenance::ScopeMaintainer;
use nexus_watcher::service::{HsEventProcessor, TEventProcessor};
use nexus_watcher::EventProcessorError;
use pubky::Keypair;
use pubky_app_specs::traits::{HasIdPath, HashId, TimestampId};
use pubky_app_specs::{
    post_uri_builder, tag_uri_builder, user_uri_builder, PubkyAppBookmark, PubkyAppPost,
    PubkyAppPostEmbed, PubkyAppPostKind, PubkyAppTag, PubkyAppUser, PubkyId,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
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
                &client()?,
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

/// A short post, replying to `parent` if given.
fn post(content: &str, parent: Option<String>) -> PubkyAppPost {
    PubkyAppPost {
        content: content.to_string(),
        kind: PubkyAppPostKind::Short,
        parent,
        embed: None,
        attachments: None,
        lock: None,
    }
}

/// The client the watcher fetches through, over the test network.
fn client() -> Result<PubkyHomeserverClient> {
    Ok(PubkyHomeserverClient::from_config(
        PubkyConnector::get()?,
        &WatcherConfig::default(),
    ))
}

/// A member's reply to an outsider's post lands at once: that one post is fetched and
/// indexed, its author stays an outsider, its other posts are not read, and the fetched
/// post stays out of the global timeline.
#[tokio_shared_rt::test(shared)]
async fn test_reply_to_an_outsider_post_fetches_only_that_post() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (outsider_kp, outsider_id) = user_on(&test, &hs_pk).await?;
    // Written to the outsider's homeserver, which the test never indexes.
    let (parent_id, _) = test
        .create_post(&outsider_kp, &post("Watcher:Scope:Parent", None))
        .await?;
    let (other_id, _) = test
        .create_post(&outsider_kp, &post("Watcher:Scope:Other", None))
        .await?;

    let replier_id = member(&mut test, "Replier").await?;
    let ingestor = scoped_ingestor(&test);
    ingestor.maybe_ingest_user(&outsider_id).await?;
    assert_user_is_outsider(&outsider_id).await;

    let reply = post(
        "Watcher:Scope:Reply",
        Some(post_uri_builder(outsider_id.to_string(), parent_id.clone())),
    );
    let reply_id = reply.create_id();
    let reply_uri = post_uri_builder(replier_id.to_string(), reply_id.clone());
    handlers::post::sync_put(
        reply,
        reply_uri,
        replier_id.clone(),
        reply_id.clone(),
        &ingestor,
        &client()?,
    )
    .await?;

    assert!(
        PostDetails::get_by_id(&replier_id, &reply_id)
            .await?
            .is_some(),
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
    assert_user_is_outsider(&outsider_id).await;
    let in_timeline = PostDetails::check_sorted_set_member(
        None,
        &POST_TIMELINE_KEY_PARTS,
        &[&outsider_id, &parent_id],
    )
    .await?;
    assert!(
        in_timeline.is_none(),
        "an outsider's post stays out of the global timeline"
    );
    Ok(())
}

/// A reply at the bottom of an outsider's thread longer than one attempt's fetch budget
/// still lands: each retry continues the chain from where the last one stopped, and no
/// attempt fetches more than the budget, whatever walks its handler starts.
#[tokio_shared_rt::test(shared)]
async fn test_a_reply_under_a_long_outsider_thread_lands_over_retries() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (outsider_kp, outsider_id) = user_on(&test, &hs_pk).await?;

    let mut parent = None;
    let mut thread = Vec::new();
    for depth in 0..25 {
        let content = format!("Watcher:Scope:Thread:{depth}");
        let (post_id, _) = test
            .create_post(&outsider_kp, &post(&content, parent.take()))
            .await?;
        parent = Some(post_uri_builder(outsider_id.to_string(), post_id.clone()));
        thread.push(post_id);
    }

    let replier_id = member(&mut test, "DeepReplier").await?;
    let ingestor = scoped_ingestor(&test);
    ingestor.maybe_ingest_user(&outsider_id).await?;

    let resume_key = format!(
        "Scope:FetchResume:{}",
        parent.clone().expect("the thread has posts")
    );
    let reply = post("Watcher:Scope:DeepReply", parent);
    let reply_id = reply.create_id();
    let reply_uri = post_uri_builder(replier_id.to_string(), reply_id.clone());
    let client = CountingFetches(client()?, AtomicU32::new(0));

    let mut attempts = 0;
    loop {
        attempts += 1;
        let fetches_before = client.1.load(Ordering::Relaxed);
        let result = handlers::post::sync_put(
            reply.clone(),
            reply_uri.clone(),
            replier_id.clone(),
            reply_id.clone(),
            &ingestor,
            &client,
        )
        .await;
        let fetched = client.1.load(Ordering::Relaxed) - fetches_before;
        assert!(fetched <= 20, "attempt {attempts} fetched {fetched} posts");
        match result {
            Ok(()) => break,
            Err(EventProcessorError::MissingDependency { .. }) if attempts < 5 => {
                if attempts == 1 {
                    let mut redis_conn = nexus_common::db::get_redis_conn().await?;
                    let ttl: i64 = deadpool_redis::redis::cmd("TTL")
                        .arg(&resume_key)
                        .query_async(&mut redis_conn)
                        .await?;
                    assert!(
                        ttl > 0,
                        "the resume point expires, so it can't pile up: {ttl}"
                    );
                }
                continue;
            }
            Err(e) => panic!("attempt {attempts}: {e:?}"),
        }
    }

    assert!(attempts > 1, "25 posts don't fit one attempt's budget");
    for post_id in &thread {
        assert!(PostDetails::get_by_id(&outsider_id, post_id)
            .await?
            .is_some());
    }
    assert!(PostDetails::get_by_id(&replier_id, &reply_id)
        .await?
        .is_some());
    Ok(())
}

/// A thread of `len` posts by `author`, each replying to the last; the uri of its last.
async fn thread(
    test: &mut WatcherTest,
    author_kp: &Keypair,
    author_id: &PubkyId,
    name: &str,
    len: usize,
) -> Result<String> {
    let mut parent = None;
    for depth in 0..len {
        let (post_id, _) = test
            .create_post(author_kp, &post(&format!("{name}:{depth}"), parent.take()))
            .await?;
        parent = Some(post_uri_builder(author_id.to_string(), post_id));
    }
    Ok(parent.expect("a thread has posts"))
}

/// A post replying under one outsider thread and reposting the end of another fetches no
/// more than one attempt's budget in all: the walk for its second target gets what the
/// first left, not a budget of its own.
#[tokio_shared_rt::test(shared)]
async fn test_one_attempt_shares_its_fetch_budget_across_targets() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (outsider_kp, outsider_id) = user_on(&test, &hs_pk).await?;
    let short = thread(&mut test, &outsider_kp, &outsider_id, "Scope:Short", 3).await?;
    let long = thread(&mut test, &outsider_kp, &outsider_id, "Scope:Long", 25).await?;
    let member_id = member(&mut test, "Scope:TwoTargets").await?;
    let ingestor = scoped_ingestor(&test);
    ingestor.maybe_ingest_user(&outsider_id).await?;

    let both = PubkyAppPost {
        embed: Some(PubkyAppPostEmbed {
            kind: PubkyAppPostKind::Short,
            uri: long,
        }),
        ..post("Scope:TwoTargets:Post", Some(short))
    };
    let post_id = both.create_id();
    let uri = post_uri_builder(member_id.to_string(), post_id.clone());
    let client = CountingFetches(client()?, AtomicU32::new(0));
    let result = handlers::post::sync_put(both, uri, member_id, post_id, &ingestor, &client).await;

    let fetched = client.1.load(Ordering::Relaxed);
    assert!(fetched <= 20, "one attempt fetched {fetched} posts");
    assert!(
        matches!(result, Err(EventProcessorError::MissingDependency { .. })),
        "the long thread continues on a retry: {result:?}"
    );
    Ok(())
}

/// A member's bookmark of a post by an outsider this Nexus doesn't know yet lands: the
/// outsider is kept out of the scope and that one post is fetched.
#[tokio_shared_rt::test(shared)]
async fn test_a_bookmark_of_an_unknown_outsiders_post_lands() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (outsider_kp, outsider_id) = user_on(&test, &hs_pk).await?;
    let (post_id, _) = test
        .create_post(&outsider_kp, &post("Watcher:Scope:Bookmarked", None))
        .await?;
    let member_id = member(&mut test, "Bookmarker").await?;
    let ingestor = scoped_ingestor(&test);

    let bookmark = PubkyAppBookmark {
        uri: post_uri_builder(outsider_id.to_string(), post_id.clone()),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let bookmark_id = bookmark.create_id();
    handlers::bookmark::sync_put(member_id, bookmark, bookmark_id, &ingestor, &client()?).await?;

    assert!(
        PostDetails::get_by_id(&outsider_id, &post_id)
            .await?
            .is_some(),
        "the bookmarked post is fetched"
    );
    assert_user_is_outsider(&outsider_id).await;
    Ok(())
}

/// A member's reply to a post by a known outsider whose homeserver is now blacklisted doesn't
/// fetch that post: the blacklist is checked for every fetch, not only on ingestion.
#[tokio_shared_rt::test(shared)]
async fn test_a_known_outsider_on_a_blacklisted_homeserver_is_not_fetched() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (outsider_kp, outsider_id) = user_on(&test, &hs_pk).await?;
    let (parent_id, _) = test
        .create_post(&outsider_kp, &post("Watcher:Scope:BlacklistedParent", None))
        .await?;
    let replier_id = member(&mut test, "BlacklistReplier").await?;
    // Known as an outsider before its homeserver was blacklisted.
    scoped_ingestor(&test)
        .maybe_ingest_user(&outsider_id)
        .await?;
    assert_user_is_outsider(&outsider_id).await;

    let scope: Arc<dyn Scope> = Arc::new(HomeserverListScope::new(std::slice::from_ref(
        &test.homeserver_id,
    )));
    let blacklisting = UserIngestor::new([PubkyId::from(hs_pk)], Arc::new(PubkyConnectorResolver))
        .with_scope(scope);
    let reply = post(
        "Watcher:Scope:ReplyToBlacklisted",
        Some(post_uri_builder(outsider_id.to_string(), parent_id.clone())),
    );
    let reply_id = reply.create_id();
    let reply_uri = post_uri_builder(replier_id.to_string(), reply_id.clone());
    let result = handlers::post::sync_put(
        reply,
        reply_uri,
        replier_id,
        reply_id,
        &blacklisting,
        &client()?,
    )
    .await;

    assert!(result.is_err(), "the reply can't land without its parent");
    assert!(
        PostDetails::get_by_id(&outsider_id, &parent_id)
            .await?
            .is_none(),
        "nothing is read from a blacklisted homeserver"
    );
    Ok(())
}

/// An outsider let into the scope gets its posts indexed while it was out into the
/// global timeline: reading its stream again wouldn't, as they are already indexed.
#[tokio_shared_rt::test(shared)]
async fn test_a_promoted_outsiders_posts_join_the_global_feeds() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (outsider_kp, outsider_id) = user_on(&test, &hs_pk).await?;
    let (parent_id, _) = test
        .create_post(&outsider_kp, &post("Watcher:Scope:OutsiderParent", None))
        .await?;
    let replier_id = member(&mut test, "PromoteReplier").await?;
    let ingestor = scoped_ingestor(&test);
    ingestor.maybe_ingest_user(&outsider_id).await?;
    let reply = post(
        "Watcher:Scope:ReplyToOutsider",
        Some(post_uri_builder(outsider_id.to_string(), parent_id.clone())),
    );
    let reply_id = reply.create_id();
    let reply_uri = post_uri_builder(replier_id.to_string(), reply_id.clone());
    handlers::post::sync_put(
        reply,
        reply_uri,
        replier_id,
        reply_id,
        &ingestor,
        &client()?,
    )
    .await?;
    let entry = [outsider_id.as_ref(), parent_id.as_str()];
    let in_timeline =
        || PostDetails::check_sorted_set_member(None, &POST_TIMELINE_KEY_PARTS, &entry);
    assert!(in_timeline().await?.is_none(), "kept out while an outsider");

    // Ranked, so the trust filter doesn't keep its posts out for its own reason.
    rank_user(&outsider_id).await?;
    let promoted = UserIngestor::new([], Arc::new(PubkyConnectorResolver))
        .promote(&outsider_id)
        .await;
    let after = in_timeline().await;
    unrank_user(&outsider_id).await?;
    catch_up::caught_up(&outsider_id).await?;

    assert!(promoted?);
    assert!(after?.is_some(), "in the global timeline once in scope");
    Ok(())
}

/// An outsider's post, fetched and indexed for a member's reply to it, with the scoped
/// ingestor that fetched it.
async fn fetched_for_a_reply(
    test: &mut WatcherTest,
    name: &str,
) -> Result<(PubkyId, String, UserIngestor)> {
    let hs_pk = create_external_test_homeserver(test).await?;
    let (outsider_kp, outsider_id) = user_on(test, &hs_pk).await?;
    let (post_id, _) = test
        .create_post(&outsider_kp, &post(&format!("{name}:Parent"), None))
        .await?;
    let replier_id = member(test, &format!("{name}:Replier")).await?;
    let ingestor = scoped_ingestor(test);
    ingestor.maybe_ingest_user(&outsider_id).await?;
    let reply = post(
        &format!("{name}:Reply"),
        Some(post_uri_builder(outsider_id.to_string(), post_id.clone())),
    );
    let reply_id = reply.create_id();
    let reply_uri = post_uri_builder(replier_id.to_string(), reply_id.clone());
    handlers::post::sync_put(
        reply,
        reply_uri,
        replier_id,
        reply_id,
        &ingestor,
        &client()?,
    )
    .await?;
    Ok((outsider_id, post_id, ingestor))
}

/// A fetched post saved only in part (in the graph, without the Redis details and
/// relationships a save writes last) is finished before a record lands on it: nothing
/// reads its author's stream to finish it otherwise.
#[tokio_shared_rt::test(shared)]
async fn test_a_fetched_post_saved_in_part_is_finished_before_a_record_lands_on_it() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (outsider_id, post_id, ingestor) =
        fetched_for_a_reply(&mut test, "Scope:SavedInPart").await?;
    PostDetails::remove_from_index_multiple_json(&[&[outsider_id.as_ref(), post_id.as_str()]])
        .await?;
    PostRelationships::delete(&outsider_id, &post_id).await?;

    let bookmarker_id = member(&mut test, "Scope:SavedInPart:Marker").await?;
    let bookmark = PubkyAppBookmark {
        uri: post_uri_builder(outsider_id.to_string(), post_id.clone()),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let bookmark_id = bookmark.create_id();
    handlers::bookmark::sync_put(bookmarker_id, bookmark, bookmark_id, &ingestor, &client()?)
        .await?;

    assert!(
        PostDetails::get_from_index(&outsider_id, &post_id)
            .await?
            .is_some(),
        "details written again"
    );
    assert!(
        PostRelationships::get_from_index(&outsider_id, &post_id)
            .await?
            .is_some(),
        "relationships too: the whole save is finished, not just the details"
    );
    Ok(())
}

/// An admission that fails after the outsider's posts went into the global feeds takes
/// them back out: it is an outsider again, and the feeds serve their entries without
/// checking the scope.
#[tokio_shared_rt::test(shared)]
async fn test_a_failed_admission_takes_the_posts_back_out() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (outsider_id, post_id, _) = fetched_for_a_reply(&mut test, "Scope:FailedAdmit").await?;
    // Ranked, so the trust filter would show its posts.
    rank_user(&outsider_id).await?;
    // The tag ranking, the step after the feeds, fails: the user's labels set isn't one.
    let labels_key = format!("Sorted:{}:{outsider_id}", USER_TAGS_KEY_PARTS.join(":"));
    let mut redis_conn = get_redis_conn().await?;
    let _: () = redis_conn.set(&labels_key, "not a sorted set").await?;

    let admitted = admit_outsider(&outsider_id).await;
    let entry = [outsider_id.as_ref(), post_id.as_str()];
    let in_timeline =
        PostDetails::check_sorted_set_member(None, &POST_TIMELINE_KEY_PARTS, &entry).await;
    let _: () = redis_conn.del(&labels_key).await?;
    unrank_user(&outsider_id).await?;

    assert!(admitted.is_err());
    assert_user_is_outsider(&outsider_id).await;
    assert!(
        in_timeline?.is_none(),
        "taken back out of the global timeline"
    );
    Ok(())
}

/// The real client, counting the resources fetched through it.
struct CountingFetches(PubkyHomeserverClient, AtomicU32);

#[async_trait::async_trait]
impl PkdnsHomeserverResolver for CountingFetches {
    async fn resolve_homeserver(
        &self,
        user_pk: &pubky::PublicKey,
    ) -> nexus_common::db::PubkyClientResult<Option<PubkyId>> {
        self.0.resolve_homeserver(user_pk).await
    }
}

#[async_trait::async_trait]
impl HomeserverClient for CountingFetches {
    async fn poll_events(
        &self,
        hs: &PubkyId,
        cursor: u64,
        limit: u16,
    ) -> Result<Vec<String>, HsError> {
        self.0.poll_events(hs, cursor, limit).await
    }

    async fn user_events(
        &self,
        hs: &PubkyId,
        user: &pubky::PublicKey,
        cursor: u64,
        limit: u16,
    ) -> Result<Vec<HsStreamEvent>, HsError> {
        self.0.user_events(hs, user, cursor, limit).await
    }

    async fn get_resource(&self, resource: &pubky::PubkyResource) -> Result<Vec<u8>, HsError> {
        self.1.fetch_add(1, Ordering::Relaxed);
        self.0.get_resource(resource).await
    }

    async fn get_blob(&self, resource: &pubky::PubkyResource) -> Result<Vec<u8>, HsError> {
        self.0.get_blob(resource).await
    }
}

/// The real client, but the moderator takes each post down while it is being fetched:
/// after the check before the fetch, before the save.
struct TakenDownWhileFetched(PubkyHomeserverClient);

#[async_trait::async_trait]
impl PkdnsHomeserverResolver for TakenDownWhileFetched {
    async fn resolve_homeserver(
        &self,
        user_pk: &pubky::PublicKey,
    ) -> nexus_common::db::PubkyClientResult<Option<PubkyId>> {
        self.0.resolve_homeserver(user_pk).await
    }
}

#[async_trait::async_trait]
impl HomeserverClient for TakenDownWhileFetched {
    async fn poll_events(
        &self,
        hs: &PubkyId,
        cursor: u64,
        limit: u16,
    ) -> Result<Vec<String>, HsError> {
        self.0.poll_events(hs, cursor, limit).await
    }

    async fn user_events(
        &self,
        hs: &PubkyId,
        user: &pubky::PublicKey,
        cursor: u64,
        limit: u16,
    ) -> Result<Vec<HsStreamEvent>, HsError> {
        self.0.user_events(hs, user, cursor, limit).await
    }

    async fn get_resource(&self, resource: &pubky::PubkyResource) -> Result<Vec<u8>, HsError> {
        record_takedown(
            &resource.to_pubky_url(),
            "pubky://moderator/pub/pubky.app/tags/T",
        )
        .await
        .expect("records the takedown");
        self.0.get_resource(resource).await
    }

    async fn get_blob(&self, resource: &pubky::PubkyResource) -> Result<Vec<u8>, HsError> {
        self.0.get_blob(resource).await
    }
}

/// An outsider's post the moderator takes down while it is being fetched for a member's
/// reply is removed again once saved, as an event's record is: it isn't left indexed.
#[tokio_shared_rt::test(shared)]
async fn test_a_post_taken_down_while_fetched_is_not_left_indexed() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (outsider_kp, outsider_id) = user_on(&test, &hs_pk).await?;
    let (parent_id, _) = test
        .create_post(&outsider_kp, &post("Watcher:Scope:TakenDownParent", None))
        .await?;
    let replier_id = member(&mut test, "TakedownReplier").await?;
    let ingestor = scoped_ingestor(&test);
    ingestor.maybe_ingest_user(&outsider_id).await?;

    let reply = post(
        "Watcher:Scope:ReplyToTakenDown",
        Some(post_uri_builder(outsider_id.to_string(), parent_id.clone())),
    );
    let reply_id = reply.create_id();
    let reply_uri = post_uri_builder(replier_id.to_string(), reply_id.clone());
    let client = TakenDownWhileFetched(client()?);
    let result =
        handlers::post::sync_put(reply, reply_uri, replier_id, reply_id, &ingestor, &client).await;

    assert!(result.is_err(), "the reply waits: its parent is gone");
    assert!(
        PostDetails::get_by_id(&outsider_id, &parent_id)
            .await?
            .is_none(),
        "the taken-down post isn't left indexed"
    );
    Ok(())
}
