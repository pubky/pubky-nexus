use super::utils::{assert_user_ingested, create_external_test_homeserver, user_on};
use crate::event_processor::utils::watcher::WatcherTest;
use crate::utils::default_moderation_tests;
use anyhow::Result;
use nexus_common::db::graph::Query;
use nexus_common::db::kv::RedisOps;
use nexus_common::db::{fetch_key_from_graph, fetch_row_from_graph, queries, PubkyConnector};
use nexus_common::models::post::PostDetails;
use nexus_common::models::scope::{HomeserverListScope, Scope};
use nexus_common::models::user::outsider::admit_outsider;
use nexus_common::models::user::TAG_GLOBAL_USER_TAGGERS;
use nexus_common::models::user::{
    is_outsider, user_hs_cursor_key, PubkyConnectorResolver, UserCounts, UserDetails, UserIngestor,
    UserStream, USER_MOSTFOLLOWED_KEY_PARTS,
};
use nexus_watcher::events::handlers;
use nexus_watcher::events::{DefaultEventHandler, Event, EventHandler, ParseResult};
use nexus_watcher::homeserver::PubkyHomeserverClient;
use pubky::Keypair;
use pubky_app_specs::traits::HashId;
use pubky_app_specs::traits::TimestampId;
use pubky_app_specs::{
    post_uri_builder, tag_uri_builder, user_uri_builder, PubkyAppPost, PubkyAppTag, PubkyAppUser,
    PubkyId,
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
    assert!(
        UserCounts::get_from_index(&user_id).await?.is_some(),
        "an outsider has counts, so records that move them don't fail"
    );

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

/// An event by an author outside the scope is skipped before it is fetched: nothing is
/// indexed and the event doesn't fail.
#[tokio_shared_rt::test(shared)]
async fn test_event_by_an_author_outside_the_scope_is_skipped() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (_, author_id) = user_on(&test, &hs_pk).await?;
    let ingestor = Arc::new(scoped_ingestor(&test));
    ingestor.maybe_ingest_user(&author_id).await?;
    assert_user_is_outsider(&author_id).await;

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

    let counts = UserCounts::get_from_index(&outsider_id)
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

/// An unknown author whose homeserver isn't listed is placed by the homeserver it
/// publishes, not by the listed feed its old event came from: the event is skipped and
/// the author kept as an outsider.
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
