//! Test harness for the migration suite: a testnet with a primary and two
//! external homeservers, nexus's indexers driven one step at a time, and user
//! actions performed through per-device SDK clients.

use std::collections::HashSet;
use std::sync::Arc;

use anyhow::{anyhow, ensure, Context, Result};
use nexus_common::db::graph::Query;
use nexus_common::db::{
    fetch_key_from_graph, fetch_row_from_graph, queries, PubkyClientError, PubkyClientResult,
    PubkyConnector,
};
use nexus_common::models::homeserver::{Homeserver, HsBlacklist};
use nexus_common::models::notification::Notification;
use nexus_common::models::user::{StaleMapping, UserHsCursor, UserIngestor};
use nexus_common::types::Pagination;
use nexus_common::utils::test_utils::default_ingestor_tests;
use nexus_common::{StackConfig, StackManager, DEFAULT_MAX_FILE_SIZE};
use nexus_watcher::events::retry::{InitialBackoff, RedisRetryStore, RetryScheduler, RetryStore};
use nexus_watcher::events::{DefaultEventHandler, EventHandler, Moderation};
use nexus_watcher::service::indexer::{
    KeyBasedEventProcessor, PubkyKeyBasedEventSource, TEventProcessor,
};
use nexus_watcher::service::runner::UserNotFoundBackoff;
use nexus_watcher::service::stats::ProcessorRunStatus;
use nexus_watcher::service::user_hs_resolver::{self, PkdnsHomeserverResolver, UnboundBackoff};
use nexus_watcher::service::{HsEventProcessorRunner, TEventProcessorRunner};
use pubky::pkarr::dns::rdata::TXT;
use pubky::pkarr::dns::Name;
use pubky::pkarr::SignedPacket;
use pubky::{Keypair, Pubky, PubkySession, PublicKey};
use pubky_app_specs::traits::{HasIdPath, HasPath, HashId, TimestampId};
use pubky_app_specs::{
    post_uri_builder, PubkyAppFollow, PubkyAppPost, PubkyAppPostKind, PubkyAppTag, PubkyAppUser,
    PubkyId,
};
use pubky_testnet::Testnet;
use tempfile::TempDir;
use tokio::sync::watch;

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
        let store: Arc<dyn RetryStore> = Arc::new(RedisRetryStore::new());
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
        self.key_based(hs.clone())
            .run()
            .await
            .map_err(|e| anyhow!("{e:?}"))
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
        let resolver = FreshLookup {
            users: HashSet::from([user.public_key().to_z32()]),
        };
        let mut shutdown_rx = self.shutdown_rx.clone();
        user_hs_resolver::run(
            &resolver,
            0,
            &UnboundBackoff::default(),
            &mut shutdown_rx,
            StaleMapping::Disabled,
        )
        .await
        .map_err(|e| anyhow!("{e}"))
    }

    /// What nexus does when a known user first references an unknown one:
    /// a stub node, a binding to the homeserver the record names, cursor 0.
    pub async fn ingest(&self, user: &Keypair) -> Result<()> {
        self.ingestor
            .maybe_ingest_user(&PubkyId::from(user.public_key()))
            .await
            .map_err(|e| anyhow!("ingesting the user: {e}"))
    }

    fn key_based(&self, hs: PubkyId) -> Arc<KeyBasedEventProcessor> {
        Arc::new(KeyBasedEventProcessor {
            homeserver_id: hs,
            limit: 100,
            event_handler: self.handler.clone(),
            event_source: Arc::new(PubkyKeyBasedEventSource),
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
        Ok(session)
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

fn is_profile(path: &str) -> bool {
    path.trim_start_matches('/') == PubkyAppUser::create_path().trim_start_matches('/')
}

pub fn post_uri(user: &Keypair, post_id: &str) -> String {
    post_uri_builder(user.public_key().to_z32(), post_id.to_string())
}
