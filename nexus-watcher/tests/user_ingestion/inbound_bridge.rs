use super::utils::{create_external_test_homeserver, user_on};
use crate::event_processor::utils::watcher::WatcherTest;
use crate::utils::default_moderation_tests;
use anyhow::Result;
use deadpool_redis::redis::AsyncCommands;
use nexus_common::db::{get_redis_conn, PubkyConnector};
use nexus_common::models::notification::{Notification, NotificationBody};
use nexus_common::models::post::PostDetails;
use nexus_common::models::scope::{Scope, SeedScope};
use nexus_common::models::user::outsider::create_outsider;
use nexus_common::models::user::{
    PkdnsHomeserverResolver, PubkyConnectorResolver, UserDetails, UserIngestor,
};
use nexus_common::types::DynError;
use nexus_common::{InboundBridgeConfig, ScopeConfig, WatcherConfig};
use nexus_watcher::homeserver::{
    HomeserverClient, HsError, HsErrorKind, HsOp, HsStreamEvent, PubkyHomeserverClient,
};
use nexus_watcher::service::big_nexus::NexusAnswer;
use nexus_watcher::service::inbound_bridge::{
    BridgeIndexing, InboundBridge, NotificationSource, Outcome,
};
use pubky::{Keypair, ResourcePath};
use pubky_app_specs::traits::{HasIdPath, TimestampId};
use pubky_app_specs::{post_uri_builder, PubkyAppPost, PubkyAppPostKind, PubkyAppUser, PubkyId};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::watch::Receiver;

/// Notifications served from memory, newest first, 100 a page, recording the `end` each
/// page asked for.
struct FakeBigNexus {
    notifications: Mutex<Vec<Notification>>,
    ends: Mutex<Vec<Option<i64>>>,
}

impl FakeBigNexus {
    fn new(notifications: Vec<Notification>) -> Self {
        Self {
            notifications: Mutex::new(notifications),
            ends: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait::async_trait]
impl NotificationSource for FakeBigNexus {
    fn name(&self) -> &str {
        "fake"
    }

    async fn page(
        &self,
        _user_id: &str,
        start: Option<i64>,
        end: Option<i64>,
        skip: usize,
    ) -> Result<NexusAnswer<Vec<Notification>>, DynError> {
        self.ends.lock().unwrap().push(end);
        let mut page: Vec<Notification> = self
            .notifications
            .lock()
            .unwrap()
            .iter()
            .filter(|n| start.is_none_or(|start| n.timestamp <= start))
            .filter(|n| end.is_none_or(|end| n.timestamp >= end))
            .map(|n| Notification {
                timestamp: n.timestamp,
                body: n.body.clone(),
            })
            .collect();
        page.sort_by_key(|n| std::cmp::Reverse(n.timestamp));
        // 100 a page, as the endpoint serves at most.
        Ok(NexusAnswer::Found(
            page.into_iter().skip(skip).take(100).collect(),
        ))
    }
}

/// The bridge of a seed scope around `seed`, reading `source`.
fn bridge(seed: &PubkyId, source: Arc<FakeBigNexus>) -> Result<InboundBridge> {
    let client =
        PubkyHomeserverClient::from_config(PubkyConnector::get()?, &WatcherConfig::default());
    Ok(bridge_with(seed, source, Arc::new(client), running()))
}

/// A shutdown signal that never fires.
fn running() -> Receiver<bool> {
    tokio::sync::watch::channel(false).1
}

/// [`bridge`] fetching records through `client`.
fn bridge_with(
    seed: &PubkyId,
    source: Arc<FakeBigNexus>,
    client: Arc<dyn HomeserverClient>,
    shutdown: Receiver<bool>,
) -> InboundBridge {
    let scope: Arc<dyn Scope> = Arc::new(SeedScope::new(seed.clone(), 1));
    let ingestor = UserIngestor::new(Vec::<PubkyId>::new(), Arc::new(PubkyConnectorResolver))
        .with_scope(scope);
    let indexing = BridgeIndexing {
        ingestor: Arc::new(ingestor),
        moderation: default_moderation_tests(),
        client,
        files_path: PathBuf::from("/tmp/nexus-watcher-test"),
    };
    InboundBridge::new(
        seed.clone(),
        vec![Box::new(Shared(source))],
        indexing,
        0,
        shutdown,
    )
}

/// The real client, but one record's homeserver answers `503`.
struct FailingFor {
    inner: PubkyHomeserverClient,
    failing: String,
}

#[async_trait::async_trait]
impl PkdnsHomeserverResolver for FailingFor {
    async fn resolve_homeserver(
        &self,
        user_pk: &pubky::PublicKey,
    ) -> nexus_common::db::PubkyClientResult<Option<PubkyId>> {
        self.inner.resolve_homeserver(user_pk).await
    }
}

#[async_trait::async_trait]
impl HomeserverClient for FailingFor {
    async fn poll_events(
        &self,
        hs: &PubkyId,
        cursor: u64,
        limit: u16,
    ) -> Result<Vec<String>, HsError> {
        self.inner.poll_events(hs, cursor, limit).await
    }

    async fn user_events(
        &self,
        hs: &PubkyId,
        user: &pubky::PublicKey,
        cursor: u64,
        limit: u16,
    ) -> Result<Vec<HsStreamEvent>, HsError> {
        self.inner.user_events(hs, user, cursor, limit).await
    }

    async fn get_resource(&self, resource: &pubky::PubkyResource) -> Result<Vec<u8>, HsError> {
        if resource.to_string() == self.failing {
            let owner = PubkyId::try_from(resource.owner.to_z32().as_str()).unwrap();
            let down = HsErrorKind::ServerError(503, "down".into());
            return Err(HsError::new(&owner, HsOp::GetResource, down));
        }
        self.inner.get_resource(resource).await
    }

    async fn get_blob(&self, resource: &pubky::PubkyResource) -> Result<Vec<u8>, HsError> {
        self.inner.get_blob(resource).await
    }
}

/// Lets a test keep a handle on the source it gave the bridge.
struct Shared(Arc<FakeBigNexus>);

#[async_trait::async_trait]
impl NotificationSource for Shared {
    fn name(&self) -> &str {
        self.0.name()
    }

    async fn page(
        &self,
        user_id: &str,
        start: Option<i64>,
        end: Option<i64>,
        skip: usize,
    ) -> Result<NexusAnswer<Vec<Notification>>, DynError> {
        self.0.page(user_id, start, end, skip).await
    }
}

/// A post on its author's homeserver, written without the watcher reading it, as nothing
/// reads an outsider.
async fn write_post(author: &Keypair, post: &PubkyAppPost) -> Result<(String, ResourcePath)> {
    let post_id = post.create_id();
    let path: ResourcePath = PubkyAppPost::create_path(&post_id).parse()?;
    let session = PubkyConnector::get()?
        .signer(author.clone())
        .signin()
        .await?;
    session
        .storage()
        .put(&path, serde_json::to_string(post)?)
        .await?;
    Ok((post_id, path))
}

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

/// The seed, indexed with a post, and an outsider on another homeserver nothing reads.
async fn seed_and_outsider(test: &mut WatcherTest) -> Result<(PubkyId, String, Keypair, PubkyId)> {
    let seed_kp = Keypair::random();
    let seed_user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Bridge:Seed".to_string(),
        status: None,
    };
    let seed = PubkyId::try_from(test.create_user(&seed_kp, &seed_user).await?.as_str())
        .map_err(anyhow::Error::msg)?;
    let (seed_post, _) = test
        .create_post(&seed_kp, &post("Bridge:SeedPost", None))
        .await?;
    let hs = create_external_test_homeserver(test).await?;
    let (outsider_kp, outsider) = user_on(test, &hs).await?;
    Ok((seed, seed_post, outsider_kp, outsider))
}

fn reply_notification(timestamp: i64, by: &PubkyId, parent: &str, reply: &str) -> Notification {
    Notification {
        timestamp,
        body: NotificationBody::Reply {
            replied_by: by.to_string(),
            parent_post_uri: parent.to_string(),
            reply_uri: reply.to_string(),
            post_kind: PubkyAppPostKind::Short,
        },
    }
}

/// Removes what a poll of `seed` stored in Redis.
async fn forget_bridge(seed: &PubkyId) -> Result<()> {
    let mut redis_conn = get_redis_conn().await?;
    let _: () = redis_conn
        .del(&[
            format!("Scope:Bridge:{seed}:fake"),
            format!("Scope:Bridge:{seed}:fake:CatchUp"),
            format!("Scope:BridgePending:{seed}"),
        ])
        .await?;
    Ok(())
}

/// An outsider's reply to the seed, which nothing reads, is indexed once a big Nexus
/// says where it is; the next poll reads only from where the last one stopped.
#[tokio_shared_rt::test(shared)]
async fn test_an_outsiders_reply_to_the_seed_is_bridged() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (seed, seed_post, outsider_kp, outsider) = seed_and_outsider(&mut test).await?;
    let parent = post_uri_builder(seed.to_string(), seed_post);
    let (reply_id, _) =
        write_post(&outsider_kp, &post("Bridge:Reply", Some(parent.clone()))).await?;
    let reply = post_uri_builder(outsider.to_string(), reply_id.clone());
    let source = Arc::new(FakeBigNexus::new(vec![reply_notification(
        1_000, &outsider, &parent, &reply,
    )]));
    let bridge = bridge(&seed, source.clone())?;

    let handled = bridge.poll_source(&Shared(source.clone())).await;
    let indexed = PostDetails::get_by_id(&outsider, &reply_id).await;
    let again = bridge.poll_source(&Shared(source.clone())).await;
    let ends = source.ends.lock().unwrap().clone();
    forget_bridge(&seed).await?;

    assert_eq!(handled.map_err(anyhow::Error::msg)?, 1);
    assert!(indexed?.is_some(), "the outsider's reply is indexed");
    again.map_err(anyhow::Error::msg)?;
    assert_eq!(
        ends,
        vec![None, Some(1_000)],
        "the next poll starts at the position"
    );
    Ok(())
}

/// A big Nexus pointing at a real record that doesn't concern the seed can't make it
/// indexed.
#[tokio_shared_rt::test(shared)]
async fn test_a_record_unrelated_to_the_seed_isnt_bridged() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (seed, _, outsider_kp, outsider) = seed_and_outsider(&mut test).await?;
    let (post_id, _) = write_post(&outsider_kp, &post("Bridge:Unrelated", None)).await?;
    let uri = post_uri_builder(outsider.to_string(), post_id.clone());
    let bridge = bridge(&seed, Arc::new(FakeBigNexus::new(Vec::new())))?;

    let outcome = bridge.bridge_record(&uri).await;
    let indexed = PostDetails::get_by_id(&outsider, &post_id).await?;
    let author = UserDetails::get_by_id(&outsider).await?;

    assert_eq!(outcome?, Outcome::Unrelated);
    assert!(indexed.is_none(), "not indexed");
    assert!(
        author.is_none(),
        "nothing written for it, not even its author"
    );
    Ok(())
}

/// A member's record is left to its own stream.
#[tokio_shared_rt::test(shared)]
async fn test_a_members_record_is_left_to_its_stream() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (seed, seed_post, _, _) = seed_and_outsider(&mut test).await?;
    let bridge = bridge(&seed, Arc::new(FakeBigNexus::new(Vec::new())))?;

    let outcome = bridge
        .bridge_record(&post_uri_builder(seed.to_string(), seed_post))
        .await?;

    assert_eq!(outcome, Outcome::Skipped);
    Ok(())
}

/// A bridged reply its author deleted is removed here too.
#[tokio_shared_rt::test(shared)]
async fn test_a_bridged_record_deleted_at_its_homeserver_is_removed() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (seed, seed_post, outsider_kp, outsider) = seed_and_outsider(&mut test).await?;
    let parent = post_uri_builder(seed.to_string(), seed_post);
    let (reply_id, path) = write_post(&outsider_kp, &post("Bridge:Deleted", Some(parent))).await?;
    let reply = post_uri_builder(outsider.to_string(), reply_id.clone());
    let bridge = bridge(&seed, Arc::new(FakeBigNexus::new(Vec::new())))?;
    assert_eq!(bridge.bridge_record(&reply).await?, Outcome::Indexed);

    let session = PubkyConnector::get()?.signer(outsider_kp).signin().await?;
    session.storage().delete(&path).await?;
    let outcome = bridge.bridge_record(&reply).await;
    let indexed = PostDetails::get_by_id(&outsider, &reply_id).await?;

    assert_eq!(outcome?, Outcome::Removed);
    assert!(
        indexed.is_none_or(|details| details.deleted),
        "removed, or a tombstone"
    );
    Ok(())
}

/// A reply to a seed's post not indexed yet waits, and is indexed by a later poll once
/// the post is.
#[tokio_shared_rt::test(shared)]
async fn test_a_reply_waiting_for_its_parent_is_bridged_later() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (seed, _, outsider_kp, outsider) = seed_and_outsider(&mut test).await?;
    // The seed's post isn't indexed yet: its stream wasn't read past it.
    let seed_kp_post = post("Bridge:LatePost", None);
    let late_id = seed_kp_post.create_id();
    let parent = post_uri_builder(seed.to_string(), late_id.clone());
    let (reply_id, _) =
        write_post(&outsider_kp, &post("Bridge:Early", Some(parent.clone()))).await?;
    let reply = post_uri_builder(outsider.to_string(), reply_id.clone());
    // A newer notification moves the position past the reply, so only the records kept
    // waiting bring it back.
    let newer = Notification {
        timestamp: 3_000,
        body: NotificationBody::Follow {
            followed_by: outsider.to_string(),
        },
    };
    let source = Arc::new(FakeBigNexus::new(vec![
        reply_notification(2_000, &outsider, &parent, &reply),
        newer,
    ]));
    let bridge = bridge(&seed, source.clone())?;

    bridge.poll().await.map_err(anyhow::Error::msg)?;
    let waiting = PostDetails::get_by_id(&outsider, &reply_id)
        .await?
        .is_none();
    // The seed's post is indexed by its own stream.
    nexus_watcher::events::handlers::post::sync_put(
        seed_kp_post,
        parent.clone(),
        seed.clone(),
        late_id,
        &UserIngestor::new(Vec::<PubkyId>::new(), Arc::new(PubkyConnectorResolver)),
        &PubkyHomeserverClient::from_config(PubkyConnector::get()?, &WatcherConfig::default()),
    )
    .await?;
    bridge.poll().await.map_err(anyhow::Error::msg)?;
    let indexed = PostDetails::get_by_id(&outsider, &reply_id).await?;
    forget_bridge(&seed).await?;

    assert!(waiting, "waits for its parent");
    assert!(indexed.is_some(), "indexed once the parent is");
    Ok(())
}

/// The bridge reads the seed's notifications, so it needs a seed scope.
#[tokio_shared_rt::test(shared)]
async fn test_a_bridge_without_a_seed_scope_stops_the_start() -> Result<()> {
    WatcherTest::setup(None).await?;
    let mut config = WatcherConfig {
        inbound_bridge: InboundBridgeConfig {
            nexus_urls: vec!["https://nexus.example".into()],
            ..Default::default()
        },
        ..Default::default()
    };
    let client = Arc::new(PubkyHomeserverClient::from_config(
        PubkyConnector::get()?,
        &config,
    ));
    let everything = InboundBridge::from_config(&config, client.clone(), running());

    config.stack.scope = ScopeConfig::Seed {
        seed: nexus_common::utils::test_utils::random_pubky_id(),
        hops: 1,
    };
    let seed = InboundBridge::from_config(&config, client, running());

    assert!(everything.is_err());
    assert!(seed.is_ok_and(|bridge| bridge.is_some()));
    Ok(())
}

/// A record whose author's homeserver is down waits on its own: the records after it are
/// still bridged and the position moves past it, and each retry that still fails counts,
/// so it is given up in time rather than holding the others forever.
#[tokio_shared_rt::test(shared)]
async fn test_a_record_on_an_unreachable_homeserver_doesnt_hold_up_the_rest() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (seed, seed_post, outsider_kp, outsider) = seed_and_outsider(&mut test).await?;
    let parent = post_uri_builder(seed.to_string(), seed_post);
    let mut replies = Vec::new();
    for content in ["Bridge:Down", "Bridge:After"] {
        let (id, _) = write_post(&outsider_kp, &post(content, Some(parent.clone()))).await?;
        replies.push((id.clone(), post_uri_builder(outsider.to_string(), id)));
    }
    let source = Arc::new(FakeBigNexus::new(vec![
        reply_notification(1_000, &outsider, &parent, &replies[0].1),
        reply_notification(2_000, &outsider, &parent, &replies[1].1),
    ]));
    let client = FailingFor {
        inner: PubkyHomeserverClient::from_config(
            PubkyConnector::get()?,
            &WatcherConfig::default(),
        ),
        // As the client sees it, which isn't the `pubky://` form.
        failing: replies[0].1.parse::<pubky::PubkyResource>()?.to_string(),
    };
    let bridge = bridge_with(&seed, source.clone(), Arc::new(client), running());

    let handled = bridge.poll_source(&Shared(source.clone())).await;
    let after = PostDetails::get_by_id(&outsider, &replies[1].0).await?;
    bridge.poll().await.map_err(anyhow::Error::msg)?;
    let mut redis_conn = get_redis_conn().await?;
    let tries: Option<u32> = redis_conn
        .hget(format!("Scope:BridgePending:{seed}"), &replies[0].1)
        .await?;
    forget_bridge(&seed).await?;

    assert_eq!(handled.map_err(anyhow::Error::msg)?, 2, "both handled");
    assert!(after.is_some(), "the next record is bridged");
    assert_eq!(tries, Some(1), "waiting, its failed retry counted");
    Ok(())
}

/// A big Nexus naming a record a notification can't be about (a profile here) can't have
/// it removed, though its homeserver doesn't have it.
#[tokio_shared_rt::test(shared)]
async fn test_a_source_cant_have_a_profile_removed() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (seed, _, _, outsider) = seed_and_outsider(&mut test).await?;
    // Known as an outsider, whose profile was never read nor written.
    create_outsider(&UserDetails::from_pubky(outsider.clone())).await?;
    let bridge = bridge(&seed, Arc::new(FakeBigNexus::new(Vec::new())))?;

    let profile = format!("pubky://{outsider}/pub/pubky.app/profile.json");
    let outcome = bridge.bridge_record(&profile).await;
    let kept = UserDetails::get_by_id(&outsider).await?;

    assert_eq!(outcome?, Outcome::Skipped);
    assert!(kept.is_some_and(|user| !user.deleted), "still there");
    Ok(())
}

/// A poll stops between records once the watcher shuts down.
#[tokio_shared_rt::test(shared)]
async fn test_a_poll_stops_when_the_watcher_shuts_down() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (seed, seed_post, outsider_kp, outsider) = seed_and_outsider(&mut test).await?;
    let parent = post_uri_builder(seed.to_string(), seed_post);
    let (reply_id, _) =
        write_post(&outsider_kp, &post("Bridge:Stopped", Some(parent.clone()))).await?;
    let reply = post_uri_builder(outsider.to_string(), reply_id.clone());
    let source = Arc::new(FakeBigNexus::new(vec![reply_notification(
        1_000, &outsider, &parent, &reply,
    )]));
    let (stop, shutdown) = tokio::sync::watch::channel(false);
    let client =
        PubkyHomeserverClient::from_config(PubkyConnector::get()?, &WatcherConfig::default());
    let bridge = bridge_with(&seed, source.clone(), Arc::new(client), shutdown);
    stop.send(true)?;

    let handled = bridge.poll_source(&Shared(source)).await;
    let indexed = PostDetails::get_by_id(&outsider, &reply_id).await?;
    forget_bridge(&seed).await?;

    assert_eq!(
        handled.map_err(anyhow::Error::msg)?,
        0,
        "stopped before the record"
    );
    assert!(indexed.is_none());
    Ok(())
}

/// A backlog longer than one poll's pages is read over the next polls: the first keeps
/// the position and a catch-up point, the next goes on from that point, then moves the
/// position to the newest notification.
#[tokio_shared_rt::test(shared)]
async fn test_a_long_backlog_is_read_over_several_polls() -> Result<()> {
    WatcherTest::setup(None).await?;
    let seed = nexus_common::utils::test_utils::random_pubky_id();
    // One more than a poll's 50 pages of 100. Followed by no valid user, so each record is
    // skipped at once: only the paging is at stake.
    let backlog = 50 * 100 + 5;
    let notifications = (1..=backlog as i64)
        .map(|timestamp| Notification {
            timestamp,
            body: NotificationBody::Follow {
                followed_by: "x".into(),
            },
        })
        .collect();
    let source = Arc::new(FakeBigNexus::new(notifications));
    let bridge = bridge(&seed, source.clone())?;
    let position_key = format!("Scope:Bridge:{seed}:fake");
    let catch_up_key = format!("{position_key}:CatchUp");
    let mut redis_conn = get_redis_conn().await?;

    let first = bridge.poll_source(&Shared(source.clone())).await;
    let position_after_first: Option<i64> = redis_conn.get(&position_key).await?;
    let catching_up: bool = redis_conn.exists(&catch_up_key).await?;
    let second = bridge.poll_source(&Shared(source)).await;
    let position: Option<i64> = redis_conn.get(&position_key).await?;
    let still_catching_up: bool = redis_conn.exists(&catch_up_key).await?;
    forget_bridge(&seed).await?;

    assert_eq!(first.map_err(anyhow::Error::msg)?, 50 * 100);
    assert_eq!(position_after_first, None, "kept until the backlog is read");
    assert!(catching_up, "a catch-up point to go on from");
    assert!(second.map_err(anyhow::Error::msg)? >= 5, "the rest read");
    assert_eq!(position, Some(backlog as i64), "the newest notification");
    assert!(!still_catching_up, "done catching up");
    Ok(())
}
