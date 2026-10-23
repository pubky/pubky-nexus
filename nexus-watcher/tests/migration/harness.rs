//! Test harness for the migration suite: a testnet with a primary and two
//! external homeservers, nexus's indexers driven one step at a time, and user
//! actions performed through per-device SDK clients.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, ensure, Context, Result};
use nexus_common::config::EventRetryConfig;
use nexus_common::db::graph::Query;
use nexus_common::db::{
    fetch_key_from_graph, fetch_row_from_graph, queries, PubkyClientError, PubkyClientResult,
    PubkyConnector,
};
use nexus_common::models::homeserver::{Homeserver, HsBlacklist};
use nexus_common::models::notification::Notification;
use nexus_common::models::user::{HomeserverRecheck, StaleMapping, UserHsCursor, UserIngestor};
use nexus_common::types::Pagination;
use nexus_common::utils::test_utils::default_ingestor_tests;
use nexus_common::{StackConfig, StackManager, DEFAULT_MAX_FILE_SIZE};
use nexus_watcher::events::retry::{
    InMemoryRetryStore, IndexKey, InitialBackoff, RetryEvent, RetryProcessor, RetryScheduler,
    RetryStore,
};
use nexus_watcher::events::{DefaultEventHandler, EventHandler, Moderation};
use nexus_watcher::service::indexer::{
    KeyBasedEventProcessor, KeyBasedEventSource, PubkyKeyBasedEventSource, TEventProcessor,
};
use nexus_watcher::service::runner::UserNotFoundBackoff;
use nexus_watcher::service::stats::ProcessorRunStatus;
use nexus_watcher::service::user_hs_resolver::{self, PkdnsHomeserverResolver, UnboundBackoff};
use nexus_watcher::service::{HsEventProcessorRunner, RunError, TEventProcessorRunner};
use nexus_watcher::EventProcessorError;
use pubky::pkarr::dns::rdata::TXT;
use pubky::pkarr::dns::Name;
use pubky::pkarr::SignedPacket;
use pubky::{Event as StreamEvent, EventCursor, Keypair, Pubky, PubkySession, PublicKey};
use pubky_app_specs::traits::{HasIdPath, HasPath, HashId, TimestampId};
use pubky_app_specs::{
    post_uri_builder, PubkyAppFollow, PubkyAppPost, PubkyAppPostKind, PubkyAppTag, PubkyAppUser,
    PubkyId,
};
use pubky_testnet::Testnet;
use tempfile::TempDir;
use tokio::sync::{watch, Notify};
use tokio::task::JoinHandle;

/// A user's `HOSTED_BY` binding as nexus stores it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub homeserver: Option<String>,
    pub stale: bool,
}

/// A user's public tree: owner-relative paths and their bytes.
pub type Tree = Vec<(String, Vec<u8>)>;

pub struct MigrationTest {
    pub testnet: Testnet,
    /// Bulk-indexed by nexus.
    pub primary: PubkyId,
    pub hs_a: PubkyId,
    pub hs_b: PubkyId,
    bulk: HsEventProcessorRunner,
    handler: Arc<dyn EventHandler>,
    ingestor: Arc<UserIngestor>,
    retry: Arc<RetryScheduler>,
    retry_queue: Arc<RetryQueue>,
    not_found_backoff: Arc<UserNotFoundBackoff>,
    shutdown_rx: watch::Receiver<bool>,
    _shutdown_tx: watch::Sender<bool>,
    _files: TempDir,
}

impl MigrationTest {
    pub async fn setup() -> Result<Self> {
        StackManager::setup(&StackConfig::default())
            .await
            .map_err(|e| anyhow!("stack setup failed: {e:?}"))?;
        let files = TempDir::new()?;

        let mut testnet = Testnet::new_unseeded().await?;
        testnet.create_http_relay().await?;
        let primary = PubkyId::from(testnet.create_random_homeserver().await?.public_key());
        let hs_a = PubkyId::from(testnet.create_random_homeserver().await?.public_key());
        let hs_b = PubkyId::from(testnet.create_random_homeserver().await?.public_key());
        Homeserver::persist_if_unknown(primary.clone())
            .await
            .map_err(|e| anyhow!("persisting the primary: {e}"))?;

        // Nexus's own client: one per process, bound to this testnet.
        PubkyConnector::init_from(testnet.sdk()?)
            .await
            .map_err(|e| anyhow!("pubky connector: {e}"))?;

        let moderation = Arc::new(Moderation {
            id: PubkyId::from(Keypair::random().public_key()),
            tags: Vec::new(),
        });
        let ingestor = default_ingestor_tests();
        let handler: Arc<dyn EventHandler> = Arc::new(DefaultEventHandler::new(
            moderation,
            ingestor.clone(),
            DEFAULT_MAX_FILE_SIZE,
            files.path().to_path_buf(),
        ));
        let retry_queue = Arc::new(RetryQueue::default());
        let store: Arc<dyn RetryStore> = retry_queue.clone();
        let retry = Arc::new(RetryScheduler::new(
            store,
            InitialBackoff {
                missing_dep_ms: 60_000,
                transient_ms: 10_000,
            },
        ));
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let bulk = HsEventProcessorRunner {
            limit: 1000,
            event_handler: handler.clone(),
            shutdown_rx: shutdown_rx.clone(),
            primary_homeserver: primary.clone(),
            retry_scheduler: retry.clone(),
        };

        Ok(Self {
            testnet,
            primary,
            hs_a,
            hs_b,
            bulk,
            handler,
            ingestor,
            retry,
            retry_queue,
            not_found_backoff: Arc::new(UserNotFoundBackoff::default()),
            shutdown_rx,
            _shutdown_tx: shutdown_tx,
            _files: files,
        })
    }

    // ----- nexus, one step at a time -----

    /// One poll of the primary's bulk stream from the stored cursor.
    pub async fn index_primary(&self) -> Result<()> {
        let stats = self.bulk.run().await.map_err(|e| anyhow!("{e}"))?;
        for run in &stats.0.stats {
            ensure!(
                run.status == ProcessorRunStatus::Ok,
                "bulk run of {} ended with {:?}",
                run.hs_id,
                run.status
            );
        }
        Ok(())
    }

    /// One per-user read of every user bound to `hs`.
    pub async fn index_external(&self, hs: &PubkyId) -> Result<()> {
        self.key_based(hs.clone(), Arc::new(PubkyKeyBasedEventSource))
            .run()
            .await
            .map_err(|e| anyhow!("{e:?}"))
    }

    /// A per-user read of `hs` that has listed its users and stops right
    /// before fetching `user`'s events, until [`PausedRead::resume`]. One read
    /// reaches the users of a busy homeserver minutes apart, so this is a read
    /// that started before a switch, caught while the switch happens.
    pub async fn index_external_paused(&self, hs: &PubkyId, user: &Keypair) -> Result<PausedRead> {
        let source = Arc::new(PausingSource {
            user: user.public_key(),
            reached: Notify::new(),
            resume: Notify::new(),
        });
        let run = tokio::spawn(self.key_based(hs.clone(), source.clone()).run());
        tokio::time::timeout(Duration::from_secs(30), source.reached.notified())
            .await
            .context("the read never reached the user")?;
        Ok(PausedRead { source, run })
    }

    /// One per-user read of the users bound to the primary: how a user's own
    /// history there is read after a switch to it.
    pub async fn catch_up(&self) -> Result<()> {
        self.index_external(&self.primary).await
    }

    /// One resolver sweep with stale mapping disabled, in which only `user` is
    /// looked up, with a fresh network lookup. Every other user in the shared
    /// graph gets a failed lookup, which the resolver leaves alone.
    pub async fn resolve(&self, user: &Keypair) -> Result<()> {
        self.sweep(&FreshLookup {
            users: HashSet::from([user.public_key().to_z32()]),
        })
        .await
    }

    /// One sweep with stale mapping disabled in which only `user` is looked
    /// up, the way the production resolver does it: cache-first, through
    /// `client`. A client other than nexus's own stands for the watcher
    /// process when the recheck runs in the API process: each process has its
    /// own pkarr cache, and the watcher's holds what its last sweep saw.
    pub async fn resolve_cached(&self, user: &Keypair, client: &Pubky) -> Result<()> {
        self.sweep(&CachedLookup {
            users: HashSet::from([user.public_key().to_z32()]),
            client: client.clone(),
        })
        .await
    }

    async fn sweep(&self, resolver: &dyn PkdnsHomeserverResolver) -> Result<()> {
        let mut shutdown_rx = self.shutdown_rx.clone();
        user_hs_resolver::run(
            resolver,
            0,
            &UnboundBackoff::default(),
            &mut shutdown_rx,
            StaleMapping::Disabled,
        )
        .await
        .map_err(|e| anyhow!("{e}"))
    }

    /// One pass of the retry processor over everything queued so far, as if
    /// every entry's backoff had passed.
    pub async fn retry(&self) -> Result<()> {
        self.retry_queue.all_due.store(true, Ordering::SeqCst);
        Arc::new(RetryProcessor {
            event_handler: self.handler.clone(),
            shutdown_rx: self.shutdown_rx.clone(),
            config: EventRetryConfig::default(),
            store: self.retry_queue.clone(),
        })
        .run()
        .await
        .map_err(|e| anyhow!("{e}"))
    }

    /// What `PUT /v0/recheck/{user}` does: a fresh lookup of the user's record,
    /// followed wherever it points.
    pub async fn recheck(&self, user: &Keypair) -> Result<HomeserverRecheck> {
        self.ingestor
            .recheck_user(&PubkyId::from(user.public_key()))
            .await
            .map_err(|e| anyhow!("rechecking the user: {e}"))?
            .ok_or_else(|| anyhow!("user unknown to nexus"))
    }

    /// What nexus does when a known user first references an unknown one:
    /// a stub node, a binding to the homeserver the record names, cursor 0.
    pub async fn ingest(&self, user: &Keypair) -> Result<()> {
        self.ingestor
            .maybe_ingest_user(&PubkyId::from(user.public_key()))
            .await
            .map_err(|e| anyhow!("ingesting the user: {e}"))
    }

    fn key_based(
        &self,
        hs: PubkyId,
        event_source: Arc<dyn KeyBasedEventSource>,
    ) -> Arc<KeyBasedEventProcessor> {
        Arc::new(KeyBasedEventProcessor {
            homeserver_id: hs,
            limit: 100,
            event_handler: self.handler.clone(),
            event_source,
            user_not_found_backoff: self.not_found_backoff.clone(),
            hs_blacklist: HsBlacklist::default(),
            retry_scheduler: self.retry.clone(),
            shutdown_rx: self.shutdown_rx.clone(),
        })
    }

    // ----- users, through their own devices -----

    /// A fresh SDK client with its own pkarr cache: one user device.
    pub fn device(&self) -> Result<Pubky> {
        Ok(self.testnet.sdk()?)
    }

    /// Signs `user` up on `hs` through `device` and writes a profile. The SDK
    /// publishes the record naming `hs`.
    pub async fn signup(
        &self,
        device: &Pubky,
        user: &Keypair,
        hs: &PubkyId,
        name: &str,
    ) -> Result<PubkySession> {
        let session = device
            .signer(user.clone())
            .signup(&hs.to_public_key(), None)
            .await?;
        self.write_profile(&session, name).await?;
        Ok(session)
    }

    pub async fn write_profile(&self, session: &PubkySession, name: &str) -> Result<()> {
        let profile = PubkyAppUser {
            name: name.to_string(),
            bio: Some("migration suite".to_string()),
            image: None,
            links: None,
            status: None,
        };
        session
            .storage()
            .put(
                PubkyAppUser::create_path().as_str(),
                serde_json::to_vec(&profile)?,
            )
            .await?;
        Ok(())
    }

    pub async fn delete_profile(&self, session: &PubkySession) -> Result<()> {
        session
            .storage()
            .delete(PubkyAppUser::create_path().as_str())
            .await?;
        Ok(())
    }

    /// A session on the homeserver `device` currently resolves for `user`.
    pub async fn session(&self, device: &Pubky, user: &Keypair) -> Result<PubkySession> {
        Ok(device.signer(user.clone()).signin().await?)
    }

    /// Writes a short post, a reply when `parent` is given. Returns the post id.
    pub async fn post(
        &self,
        session: &PubkySession,
        text: &str,
        parent: Option<String>,
    ) -> Result<String> {
        let post = PubkyAppPost::new(
            text.to_string(),
            PubkyAppPostKind::Short,
            parent,
            None,
            None,
        );
        let id = post.create_id();
        session
            .storage()
            .put(
                PubkyAppPost::create_path(&id).as_str(),
                serde_json::to_vec(&post)?,
            )
            .await?;
        // Post ids are timestamps: keep consecutive posts distinct.
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        Ok(id)
    }

    pub async fn follow(&self, session: &PubkySession, target: &str) -> Result<()> {
        let follow = PubkyAppFollow {
            created_at: chrono::Utc::now().timestamp_millis(),
        };
        session
            .storage()
            .put(
                PubkyAppFollow::create_path(target).as_str(),
                serde_json::to_vec(&follow)?,
            )
            .await?;
        Ok(())
    }

    pub async fn tag(&self, session: &PubkySession, uri: String, label: &str) -> Result<()> {
        let tag = PubkyAppTag::new(uri, label.to_string());
        let path = PubkyAppTag::create_path(&tag.create_id());
        session
            .storage()
            .put(path.as_str(), serde_json::to_vec(&tag)?)
            .await?;
        Ok(())
    }

    /// Every public `pubky.app` resource of `user` as `source` sees it: the
    /// read goes to whichever homeserver `source`'s cache names. The profile
    /// comes first, as a copy would write it.
    pub async fn public_tree(&self, source: &Pubky, user: &Keypair) -> Result<Tree> {
        let root = format!("pubky://{}/pub/pubky.app/", user.public_key().to_z32());
        let resources = source
            .public_storage()
            .list(root.as_str())?
            .limit(1000)
            .send()
            .await?;
        let mut tree = Vec::with_capacity(resources.len());
        for resource in resources {
            let bytes = source
                .public_storage()
                .get(resource.to_pubky_url().as_str())
                .await?
                .bytes()
                .await?;
            tree.push((resource.path.as_str().to_string(), bytes.to_vec()));
        }
        tree.sort_by_key(|(path, _)| (!is_profile(path), path.clone()));
        Ok(tree)
    }

    /// Writes `tree` through `session`: the copy step of a migration.
    pub async fn write_tree(&self, session: &PubkySession, tree: &Tree) -> Result<()> {
        for (path, bytes) in tree {
            session.storage().put(path.as_str(), bytes.clone()).await?;
        }
        Ok(())
    }

    /// Deletes every path in `tree` through `session`, the profile last.
    pub async fn delete_tree(&self, session: &PubkySession, tree: &Tree) -> Result<()> {
        for (path, _) in tree.iter().rev() {
            session.storage().delete(path.as_str()).await?;
        }
        Ok(())
    }

    /// Publishes the user's record naming `hs`, from `device`.
    pub async fn switch_record(
        &self,
        device: &Pubky,
        user: &Keypair,
        hs: &PublicKey,
    ) -> Result<()> {
        device
            .signer(user.clone())
            .pkdns()
            .publish_homeserver_force(Some(hs))
            .await
            .context("publishing the record")?;
        Ok(())
    }

    /// Publishes a packet for `user` with no `_pubky` record at all.
    pub async fn publish_without_homeserver(&self, device: &Pubky, user: &Keypair) -> Result<()> {
        let packet = SignedPacket::builder()
            .txt(
                Name::new("_lab")?,
                TXT::new().with_string("no homeserver here")?,
                300,
            )
            .sign(user)?;
        device
            .client()
            .pkarr()
            .publish(&packet, None)
            .await
            .context("publishing the packet")?;
        Ok(())
    }

    // ----- what nexus holds -----

    pub async fn binding(&self, user: &Keypair) -> Result<Binding> {
        let row = fetch_row_from_graph(queries::get::get_user_homeserver(
            &user.public_key().to_z32(),
        ))
        .await?;
        Ok(match row {
            None => Binding {
                homeserver: None,
                stale: false,
            },
            Some(row) => Binding {
                homeserver: row.get("homeserver_id")?,
                stale: row.get("stale")?,
            },
        })
    }

    pub async fn bound_to(&self, user: &Keypair) -> Result<Option<String>> {
        Ok(self.binding(user).await?.homeserver)
    }

    /// [`Self::bound_to`] by name, for readable assertions: `primary`, `hs_a`,
    /// `hs_b`, `elsewhere` or `nowhere`.
    pub async fn bound_name(&self, user: &Keypair) -> Result<&'static str> {
        Ok(match self.bound_to(user).await? {
            None => "nowhere",
            Some(hs) if hs == self.primary.as_ref() => "primary",
            Some(hs) if hs == self.hs_a.as_ref() => "hs_a",
            Some(hs) if hs == self.hs_b.as_ref() => "hs_b",
            Some(_) => "elsewhere",
        })
    }

    /// The user's per-user cursor on `hs`, 0 when none is stored.
    pub async fn cursor(&self, user: &Keypair, hs: &PubkyId) -> Result<u64> {
        let user_id = user.public_key().to_z32();
        let cursors = UserHsCursor::read(&[user_id.as_str()], hs.as_ref()).await?;
        Ok(cursors.into_iter().next().unwrap_or_default())
    }

    /// The bulk cursor on the primary.
    pub async fn bulk_cursor(&self) -> Result<u64> {
        Ok(Homeserver::get_from_index(self.primary.as_ref())
            .await?
            .ok_or_else(|| anyhow!("primary missing from the index"))?
            .cursor)
    }

    /// Non-deleted posts of `user` in the graph.
    pub async fn live_posts(&self, user: &Keypair) -> Result<i64> {
        let query = Query::new(
            "migration_live_posts",
            "MATCH (u:User {id: $id})-[:AUTHORED]->(p:Post)
             WHERE NOT coalesce(p.deleted, false)
             RETURN count(p) AS n",
        )
        .param("id", user.public_key().to_z32());
        Ok(fetch_key_from_graph(query, "n").await?.unwrap_or(0))
    }

    /// `(deleted, name)` of the user node.
    pub async fn profile_state(&self, user: &Keypair) -> Result<(bool, String)> {
        let row = fetch_row_from_graph(
            Query::new(
                "migration_profile_state",
                "MATCH (u:User {id: $id})
                 RETURN coalesce(u.deleted, false) AS deleted, coalesce(u.name, '') AS name",
            )
            .param("id", user.public_key().to_z32()),
        )
        .await?
        .ok_or_else(|| anyhow!("user node missing"))?;
        Ok((row.get("deleted")?, row.get("name")?))
    }

    /// The notification kinds of `user`, oldest first.
    pub async fn notifications(&self, user: &Keypair) -> Result<Vec<String>> {
        let pagination = Pagination {
            limit: Some(1000),
            ..Pagination::default()
        };
        let mut kinds: Vec<String> =
            Notification::get_by_id(&user.public_key().to_z32(), pagination)
                .await?
                .into_iter()
                .map(|n| {
                    serde_json::to_value(&n.body)
                        .ok()
                        .and_then(|v| v.get("type").and_then(|t| t.as_str()).map(str::to_string))
                        .unwrap_or_default()
                })
                .collect();
        kinds.reverse();
        Ok(kinds)
    }
}

/// A resolver for the sweep that looks up only this test's users, each with a
/// fresh network lookup that also refreshes nexus's pkarr cache, and fails the
/// lookup of everyone else so the shared graph is left untouched.
struct FreshLookup {
    users: HashSet<String>,
}

#[async_trait::async_trait]
impl PkdnsHomeserverResolver for FreshLookup {
    async fn resolve_homeserver(&self, user_pk: &PublicKey) -> PubkyClientResult<Option<PubkyId>> {
        if !self.users.contains(&user_pk.to_z32()) {
            return Err(PubkyClientError::RequestFailed {
                message: "not a user of this test".to_string(),
            });
        }
        let pubky = PubkyConnector::get()?;
        pubky.client().pkarr().resolve_most_recent(user_pk).await;
        Ok(pubky.get_homeserver_of(user_pk).await.map(PubkyId::from))
    }
}

/// A resolver for the sweep that looks up this test's users as the production
/// resolver does, with `get_homeserver_of`, but through a client of its own:
/// pkarr's `resolve` answers from that client's cache, even with an expired
/// packet, and refreshes it in the background.
struct CachedLookup {
    users: HashSet<String>,
    client: Pubky,
}

#[async_trait::async_trait]
impl PkdnsHomeserverResolver for CachedLookup {
    async fn resolve_homeserver(&self, user_pk: &PublicKey) -> PubkyClientResult<Option<PubkyId>> {
        if !self.users.contains(&user_pk.to_z32()) {
            return Err(PubkyClientError::RequestFailed {
                message: "not a user of this test".to_string(),
            });
        }
        Ok(self
            .client
            .get_homeserver_of(user_pk)
            .await
            .map(PubkyId::from))
    }
}

/// The event source of [`MigrationTest::index_external_paused`]: holds the
/// fetch of one user's events until resumed.
struct PausingSource {
    user: PublicKey,
    reached: Notify,
    resume: Notify,
}

#[async_trait::async_trait]
impl KeyBasedEventSource for PausingSource {
    async fn fetch_events(
        &self,
        hs_pk: &PublicKey,
        user_pk: &PublicKey,
        cursor: EventCursor,
        limit: u16,
    ) -> Result<Vec<StreamEvent>, EventProcessorError> {
        if *user_pk == self.user {
            self.reached.notify_one();
            self.resume.notified().await;
        }
        PubkyKeyBasedEventSource
            .fetch_events(hs_pk, user_pk, cursor, limit)
            .await
    }
}

/// A per-user read stopped before one user's fetch.
pub struct PausedRead {
    source: Arc<PausingSource>,
    run: JoinHandle<Result<(), RunError>>,
}

impl PausedRead {
    /// Lets the read fetch the user's events now and finish.
    pub async fn resume(self) -> Result<()> {
        self.source.resume.notify_one();
        self.run.await?.map_err(|e| anyhow!("{e:?}"))
    }
}

/// The suite's retry queue: in memory, so tests running in parallel processes
/// never see each other's entries, and with a switch that makes every entry
/// due for one pass of the retry processor.
#[derive(Default)]
struct RetryQueue {
    entries: InMemoryRetryStore,
    all_due: AtomicBool,
}

#[async_trait::async_trait]
impl RetryStore for RetryQueue {
    async fn put(&self, event: &RetryEvent) -> Result<(), EventProcessorError> {
        self.entries.put(event).await
    }

    async fn get(&self, index_key: &IndexKey) -> Result<Option<RetryEvent>, EventProcessorError> {
        self.entries.get(index_key).await
    }

    async fn remove(&self, index_key: &IndexKey) -> Result<(), EventProcessorError> {
        self.entries.remove(index_key).await
    }

    async fn remove_if(
        &self,
        index_key: &IndexKey,
        expected_nonce: i64,
    ) -> Result<bool, EventProcessorError> {
        self.entries.remove_if(index_key, expected_nonce).await
    }

    async fn put_if(
        &self,
        event: &RetryEvent,
        expected_nonce: i64,
    ) -> Result<bool, EventProcessorError> {
        self.entries.put_if(event, expected_nonce).await
    }

    /// The first fetch after [`MigrationTest::retry`] takes every entry; the
    /// processor's next fetch sees only what is really due, which ends the pass.
    async fn fetch_ready(
        &self,
        now: i64,
        limit: Option<usize>,
    ) -> Result<Vec<(IndexKey, RetryEvent)>, EventProcessorError> {
        let now = match self.all_due.swap(false, Ordering::SeqCst) {
            true => i64::MAX,
            false => now,
        };
        self.entries.fetch_ready(now, limit).await
    }
}

fn is_profile(path: &str) -> bool {
    path.trim_start_matches('/') == PubkyAppUser::create_path().trim_start_matches('/')
}

pub fn post_uri(user: &Keypair, post_id: &str) -> String {
    post_uri_builder(user.public_key().to_z32(), post_id.to_string())
}
