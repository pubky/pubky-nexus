use super::utils::{assert_user_ingested, create_external_test_homeserver, short_post, user_on};
use crate::event_processor::utils::watcher::{rank_user, unrank_user, WatcherTest};
use crate::utils::default_moderation_tests;
use anyhow::Result;
use nexus_common::db::graph::Query;
use nexus_common::db::kv::RedisOps;
use nexus_common::db::{fetch_key_from_graph, fetch_row_from_graph, queries, PubkyConnector};
use nexus_common::models::post::{PostDetails, POST_TIMELINE_KEY_PARTS};
use nexus_common::models::scope::{HomeserverListScope, Scope};
use nexus_common::models::traits::Collection;
use nexus_common::models::user::stub::mark_stub;
use nexus_common::models::user::{
    is_stub, set_user_homeserver, user_hs_cursor_key, UserCounts, UserDetails, UserIngestor,
    SCOPE_STATE_LEFT, USER_MOSTFOLLOWED_KEY_PARTS,
};
use nexus_common::utils::test_utils::random_pubky_id;
use nexus_common::DEFAULT_MAX_FILE_SIZE;
use nexus_watcher::events::handlers;
use nexus_watcher::events::{DefaultEventHandler, Event, EventHandler, ParseResult};
use nexus_watcher::EventProcessorError;
use pubky::Keypair;
use pubky_app_specs::traits::{HasIdPath, HashId, TimestampId};
use pubky_app_specs::{post_uri_builder, PubkyAppBookmark, PubkyAppPost, PubkyAppUser, PubkyId};
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

/// An author bound to a homeserver outside the list is retried, not skipped, until
/// scope maintenance takes it out; after that its events are skipped before they are
/// fetched: nothing is indexed and the event doesn't fail.
#[tokio_shared_rt::test(shared)]
async fn test_event_by_an_author_outside_the_scope_is_skipped() -> Result<()> {
    let test = WatcherTest::setup(None).await?;
    let handler = DefaultEventHandler::new(
        default_moderation_tests(),
        Arc::new(scoped_ingestor(&test)),
        DEFAULT_MAX_FILE_SIZE,
        PathBuf::from("/tmp/nexus-watcher-test"),
    );

    let author_id = random_pubky_id();
    UserDetails::from_pubky(author_id.clone())
        .put_to_graph()
        .await?;
    set_user_homeserver(&author_id, &random_pubky_id()).await?;

    // Never written to any homeserver: fetching it would fail, so `Ok` means skipped.
    let post_id = PubkyAppPost::default().create_id();
    let line = format!(
        "PUT {}",
        post_uri_builder(author_id.to_string(), post_id.clone())
    );
    let ParseResult::Parsed(event) = Event::parse_event(&line)? else {
        panic!("a post line parses");
    };

    let unsettled = handler.handle(&event).await;
    assert!(
        matches!(unsettled, Err(EventProcessorError::ScopeUnsettled(_))),
        "still bound and polled: retried, not lost ({unsettled:?})"
    );

    scoped_ingestor(&test).demote(&author_id).await?;
    handler.handle(&event).await?;

    assert!(PostDetails::get_by_id(&author_id, &post_id)
        .await?
        .is_none());
    Ok(())
}

/// A stub whose event comes in from a listed homeserver's feed may have moved there:
/// it is let in, and the event is indexed rather than skipped.
#[tokio_shared_rt::test(shared)]
async fn test_a_stub_that_moved_onto_a_listed_homeserver_is_let_in() -> Result<()> {
    let test = WatcherTest::setup(None).await?;
    let handler = DefaultEventHandler::new(
        default_moderation_tests(),
        Arc::new(scoped_ingestor(&test)),
        DEFAULT_MAX_FILE_SIZE,
        PathBuf::from("/tmp/nexus-watcher-test"),
    );

    let (user_kp, user_id) = user_on(&test, &test.homeserver_id.to_public_key()).await?;
    UserDetails::from_pubky(user_id.clone())
        .put_to_graph()
        .await?;
    mark_stub(&user_id).await?;

    // Written straight to the homeserver, so no event is processed yet.
    let post_id = PubkyAppPost::default().create_id();
    let post_path: pubky::ResourcePath = PubkyAppPost::create_path(&post_id).parse()?;
    let session = PubkyConnector::get()?
        .signer(user_kp.clone())
        .signin()
        .await?;
    let post = short_post("moved in", None);
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

    let query = queries::get::get_user_scope_state(&user_id);
    let scope_state: Option<Option<String>> = fetch_key_from_graph(query, "scope_state").await?;
    assert!(PostDetails::get_by_id(&user_id, &post_id).await?.is_some());
    assert!(!is_stub(&user_id).await?);
    assert_eq!(scope_state.flatten(), None, "in scope");
    Ok(())
}

/// A member's reply to a stub's post lands at once: that one post is fetched and
/// indexed, its author stays a stub, its other posts are not read, and the fetched
/// post stays out of the global timeline.
#[tokio_shared_rt::test(shared)]
async fn test_reply_to_a_stub_post_fetches_only_that_post() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (outsider_kp, outsider_id) = user_on(&test, &hs_pk).await?;
    // Written to the outsider's homeserver, which the test never indexes.
    let (parent_id, _) = test
        .create_post(&outsider_kp, &short_post("Watcher:Scope:Parent", None))
        .await?;
    let (other_id, _) = test
        .create_post(&outsider_kp, &short_post("Watcher:Scope:Other", None))
        .await?;

    let replier_id = member(&mut test, "Replier").await?;
    let ingestor = scoped_ingestor(&test);
    ingestor.maybe_ingest_user(&outsider_id).await?;
    assert_user_is_stub(&outsider_id).await;

    let reply = short_post(
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
    assert_user_is_stub(&outsider_id).await;

    let in_timeline = PostDetails::check_sorted_set_member(
        None,
        &POST_TIMELINE_KEY_PARTS,
        &[&outsider_id, &parent_id],
    )
    .await?;
    assert!(
        in_timeline.is_none(),
        "a stub's post stays out of the global timeline"
    );
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

/// A reply at the bottom of a stub thread longer than one attempt's fetch budget
/// still lands: each retry continues the chain from where the last one stopped.
#[tokio_shared_rt::test(shared)]
async fn test_a_reply_under_a_long_stub_thread_lands_over_retries() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (outsider_kp, outsider_id) = user_on(&test, &hs_pk).await?;

    let mut parent = None;
    let mut thread = Vec::new();
    for depth in 0..25 {
        let (post_id, _) = test
            .create_post(
                &outsider_kp,
                &short_post(&format!("Watcher:Scope:Thread:{depth}"), parent.take()),
            )
            .await?;
        parent = Some(post_uri_builder(outsider_id.to_string(), post_id.clone()));
        thread.push(post_id);
    }

    let replier_id = member(&mut test, "DeepReplier").await?;
    let ingestor = scoped_ingestor(&test);
    ingestor.maybe_ingest_user(&outsider_id).await?;

    let reply = short_post("Watcher:Scope:DeepReply", parent);
    let reply_id = reply.create_id();
    let reply_uri = post_uri_builder(replier_id.to_string(), reply_id.clone());

    let mut attempts = 0;
    loop {
        attempts += 1;
        match handlers::post::sync_put(
            reply.clone(),
            reply_uri.clone(),
            replier_id.clone(),
            reply_id.clone(),
            &ingestor,
        )
        .await
        {
            Ok(()) => break,
            Err(EventProcessorError::MissingDependency { .. }) if attempts < 5 => continue,
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

/// Narrowing a homeserver list lets the users on the dropped homeserver go; listing it
/// again brings them back, although they left rather than being stubs.
#[tokio_shared_rt::test(shared)]
async fn test_listing_a_homeserver_again_brings_its_users_back() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (_, user_id) = user_on(&test, &hs_pk).await?;
    let other_hs = PubkyId::from(hs_pk);
    let maintainer = |homeservers: Vec<PubkyId>| {
        let mut config = nexus_common::WatcherConfig::default();
        config.stack.scope = nexus_common::ScopeConfig::Homeservers {
            homeservers: homeservers.clone(),
        };
        let scope: Arc<dyn Scope> = Arc::new(HomeserverListScope::new(&homeservers));
        let ingestor = UserIngestor::with_scope(Vec::<PubkyId>::new(), scope);
        let limit_to = Some(vec![user_id.to_string()]);
        (
            ingestor.clone(),
            nexus_watcher::service::scope_maintenance::ScopeMaintainer::new(
                &config, ingestor, limit_to,
            ),
        )
    };
    let scope_state = || async {
        let query = queries::get::get_user_scope_state(&user_id);
        let state: Option<Option<String>> = fetch_key_from_graph(query, "scope_state").await?;
        Ok::<_, anyhow::Error>(state.flatten())
    };

    let both = vec![test.homeserver_id.clone(), other_hs.clone()];
    let (ingestor, wide) = maintainer(both.clone());
    ingestor.maybe_ingest_user(&user_id).await?;
    assert_eq!(scope_state().await?, None, "in scope");

    let (_, narrow) = maintainer(vec![test.homeserver_id.clone()]);
    narrow.at_start().await?;
    assert_eq!(scope_state().await?.as_deref(), Some(SCOPE_STATE_LEFT));

    let change = wide.at_start().await?;
    assert_eq!(change.joined, 1);
    assert_eq!(scope_state().await?, None, "back in scope");
    Ok(())
}

/// A member's bookmark of a post by an outsider this Nexus doesn't know yet lands: the
/// outsider is kept as a stub and that one post is fetched.
#[tokio_shared_rt::test(shared)]
async fn test_a_bookmark_of_an_unknown_outsiders_post_lands() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (outsider_kp, outsider_id) = user_on(&test, &hs_pk).await?;
    let (post_id, _) = test
        .create_post(&outsider_kp, &short_post("Watcher:Scope:Bookmarked", None))
        .await?;
    let member_id = member(&mut test, "Bookmarker").await?;
    let ingestor = scoped_ingestor(&test);

    let bookmark = PubkyAppBookmark {
        uri: post_uri_builder(outsider_id.to_string(), post_id.clone()),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let bookmark_id = bookmark.create_id();
    handlers::bookmark::sync_put(member_id, bookmark, bookmark_id, &ingestor).await?;

    assert!(
        PostDetails::get_by_id(&outsider_id, &post_id)
            .await?
            .is_some(),
        "the bookmarked post is fetched"
    );
    assert_user_is_stub(&outsider_id).await;
    Ok(())
}

/// A stub brought into the scope gets its posts indexed while it was a stub into the
/// global timeline: reading its stream again wouldn't, as they are already indexed.
#[tokio_shared_rt::test(shared)]
async fn test_a_promoted_stubs_posts_join_the_global_feeds() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (outsider_kp, outsider_id) = user_on(&test, &hs_pk).await?;
    let (parent_id, _) = test
        .create_post(&outsider_kp, &short_post("Watcher:Scope:StubParent", None))
        .await?;
    let replier_id = member(&mut test, "PromoteReplier").await?;
    let ingestor = scoped_ingestor(&test);
    ingestor.maybe_ingest_user(&outsider_id).await?;
    let reply = short_post(
        "Watcher:Scope:ReplyToStub",
        Some(post_uri_builder(outsider_id.to_string(), parent_id.clone())),
    );
    let reply_id = reply.create_id();
    let reply_uri = post_uri_builder(replier_id.to_string(), reply_id.clone());
    handlers::post::sync_put(reply, reply_uri, replier_id, reply_id, &ingestor).await?;
    let member = [outsider_id.as_ref(), parent_id.as_str()];
    let in_timeline =
        || PostDetails::check_sorted_set_member(None, &POST_TIMELINE_KEY_PARTS, &member);
    assert!(in_timeline().await?.is_none(), "kept out while a stub");

    // Ranked, so the trust filter doesn't keep its posts out for its own reason.
    rank_user(&outsider_id).await?;
    let promoted = UserIngestor::default().promote(&outsider_id).await;
    let after = in_timeline().await;
    unrank_user(&outsider_id).await?;

    assert!(promoted?);
    assert!(after?.is_some(), "in the global timeline once in scope");
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
    UserDetails::from_pubky(late.clone()).put_to_graph().await?;
    mark_stub(&late).await?;

    let homeservers = vec![test.homeserver_id.clone(), PubkyId::from(hs_pk.clone())];
    let mut config = nexus_common::WatcherConfig::default();
    config.stack.scope = nexus_common::ScopeConfig::Homeservers {
        homeservers: homeservers.clone(),
    };
    config.scope_maintenance.interval_ms = 0;
    let scope: Arc<dyn Scope> = Arc::new(HomeserverListScope::new(&homeservers));
    let ingestor = UserIngestor::with_scope(Vec::<PubkyId>::new(), scope);
    let maintainer = nexus_watcher::service::scope_maintenance::ScopeMaintainer::new(
        &config,
        ingestor,
        Some(vec![late.to_string()]),
    );

    let change = maintainer.at_start().await?;
    assert_eq!(change.joined, 0, "no homeserver found yet");

    test.register_user_in_hs(&late_kp, &hs_pk).await?;
    let change = maintainer.tick().await?;

    assert_eq!(change.joined, 1, "found and admitted on the next interval");
    assert!(!is_stub(&late).await?);
    Ok(())
}

/// A member's reply to a post by a known stub whose homeserver is now blacklisted doesn't
/// fetch that post: the blacklist is checked for every direct fetch, not only on ingestion.
#[tokio_shared_rt::test(shared)]
async fn test_a_known_stub_on_a_blacklisted_homeserver_is_not_fetched() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (outsider_kp, outsider_id) = user_on(&test, &hs_pk).await?;
    let (parent_id, _) = test
        .create_post(
            &outsider_kp,
            &short_post("Watcher:Scope:BlacklistedParent", None),
        )
        .await?;
    let replier_id = member(&mut test, "BlacklistReplier").await?;
    // Known as a stub before its homeserver was blacklisted.
    scoped_ingestor(&test)
        .maybe_ingest_user(&outsider_id)
        .await?;
    assert_user_is_stub(&outsider_id).await;

    let scope: Arc<dyn Scope> = Arc::new(HomeserverListScope::new(std::slice::from_ref(
        &test.homeserver_id,
    )));
    let blacklisting = UserIngestor::with_scope([PubkyId::from(hs_pk)], scope);
    let reply = short_post(
        "Watcher:Scope:ReplyToBlacklisted",
        Some(post_uri_builder(outsider_id.to_string(), parent_id.clone())),
    );
    let reply_id = reply.create_id();
    let reply_uri = post_uri_builder(replier_id.to_string(), reply_id.clone());
    let result =
        handlers::post::sync_put(reply, reply_uri, replier_id, reply_id, &blacklisting).await;

    assert!(result.is_err(), "the reply can't land without its parent");
    assert!(
        PostDetails::get_by_id(&outsider_id, &parent_id)
            .await?
            .is_none(),
        "nothing is read from a blacklisted homeserver"
    );
    Ok(())
}

/// A stub whose Redis entry is missing (the Redis half of marking it failed) is still
/// treated as one: a member's reply to its post fetches the post, and the entry is put
/// back.
#[tokio_shared_rt::test(shared)]
async fn test_a_stub_missing_from_redis_is_repaired_by_a_targeted_fetch() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (outsider_kp, outsider_id) = user_on(&test, &hs_pk).await?;
    let (parent_id, _) = test
        .create_post(
            &outsider_kp,
            &short_post("Watcher:Scope:RepairParent", None),
        )
        .await?;
    let replier_id = member(&mut test, "RepairReplier").await?;
    // A stub in the graph only.
    UserDetails::from_pubky(outsider_id.clone())
        .put_to_graph()
        .await?;
    let query = queries::put::set_user_scope_state(&outsider_id, Some("stub"));
    nexus_common::db::exec_single_row(query).await?;
    assert!(!is_stub(&outsider_id).await?);

    let ingestor = scoped_ingestor(&test);
    let reply = short_post(
        "Watcher:Scope:ReplyToRepaired",
        Some(post_uri_builder(outsider_id.to_string(), parent_id.clone())),
    );
    let reply_id = reply.create_id();
    let reply_uri = post_uri_builder(replier_id.to_string(), reply_id.clone());
    handlers::post::sync_put(reply, reply_uri, replier_id, reply_id, &ingestor).await?;

    assert!(PostDetails::get_by_id(&outsider_id, &parent_id)
        .await?
        .is_some());
    assert!(is_stub(&outsider_id).await?, "back in Scope:Stubs");
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
    let ingestor = UserIngestor::default();
    ingestor.maybe_ingest_user(&user_id).await?;
    ingestor.demote(&user_id).await?;
    // What a poll still running when the user left writes back.
    nexus_common::models::user::UserHsCursor::write(&user_id, &hs_id, 500).await?;

    assert!(ingestor.promote(&user_id).await?);

    let cursors =
        nexus_common::models::user::UserHsCursor::read(&[user_id.as_ref()], &hs_id).await?;
    assert_eq!(cursors, vec![0], "read from the start");
    Ok(())
}
