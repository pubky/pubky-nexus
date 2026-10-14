use super::utils::{assert_user_ingested, create_external_test_homeserver, short_post, user_on};
use crate::event_processor::utils::watcher::WatcherTest;
use crate::utils::default_moderation_tests;
use anyhow::Result;
use nexus_common::db::graph::Query;
use nexus_common::db::kv::RedisOps;
use nexus_common::db::{fetch_key_from_graph, fetch_row_from_graph, queries};
use nexus_common::models::post::{PostDetails, POST_TIMELINE_KEY_PARTS};
use nexus_common::models::scope::{HomeserverListScope, Scope};
use nexus_common::models::traits::Collection;
use nexus_common::models::user::{
    is_stub, set_user_homeserver, user_hs_cursor_key, UserCounts, UserDetails, UserIngestor,
    USER_MOSTFOLLOWED_KEY_PARTS,
};
use nexus_common::utils::test_utils::random_pubky_id;
use nexus_common::DEFAULT_MAX_FILE_SIZE;
use nexus_watcher::events::handlers;
use nexus_watcher::events::{DefaultEventHandler, Event, EventHandler, ParseResult};
use nexus_watcher::EventProcessorError;
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
