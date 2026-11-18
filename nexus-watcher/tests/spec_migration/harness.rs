//! Test harness for the spec migration suite: a testnet with a primary and an external
//! homeserver, nexus's indexers driven one step at a time, and users acting through their own
//! SDK sessions, writing `pubky.app` objects before the migration and `social/v1` objects after.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use nexus_common::config::EventRetryConfig;
use nexus_common::db::PubkyConnector;
use nexus_common::media::file_folder;
use nexus_common::models::follow::{Followers, Following, UserFollows};
use nexus_common::models::homeserver::{Homeserver, HsBlacklist};
use nexus_common::models::notification::Notification;
use nexus_common::models::post::{PostStream, PostView};
use nexus_common::models::tag::TagDetails;
use nexus_common::models::user::{Epoch, UserIngestor, UserView};
use nexus_common::types::Pagination;
use nexus_common::utils::test_utils::default_ingestor_tests;
use nexus_common::{StackConfig, StackManager, DEFAULT_MAX_FILE_SIZE};
use nexus_watcher::events::retry::{
    InMemoryRetryStore, IndexKey, InitialBackoff, RetryEvent, RetryProcessor, RetryScheduler,
    RetryStore,
};
use nexus_watcher::events::{DefaultEventHandler, EventHandler, Moderation};
use nexus_watcher::homeserver::{HomeserverClient, PubkyHomeserverClient};
use nexus_watcher::service::indexer::{KeyBasedEventProcessor, TEventProcessor};
use nexus_watcher::service::runner::{UserNotFoundBackoff, UserResumePoints};
use nexus_watcher::service::{HsEventProcessorRunner, TEventProcessorRunner, KEY_BASED_RUN_BUDGET};
use nexus_watcher::EventProcessorError;
use pubky::{ClientId, Keypair, PubkySession};
use pubky_social_specs::legacy_v0::traits::{HasIdPath, HasPath, HashId, TimestampId};
use pubky_social_specs::legacy_v0::{
    self, PubkyAppBlob, PubkyAppBookmark, PubkyAppFile, PubkyAppFollow, PubkyAppPost,
    PubkyAppPostEmbed, PubkyAppPostKind, PubkyAppTag, PubkyAppUser,
};
use pubky_social_specs::traits::{HasIdPath as _, HasPath as _, HashId as _};
use pubky_social_specs::{
    PubkyId, PubkySocialFollow, PubkySocialPost, PubkySocialPostKind, PubkySocialTag,
    PubkySocialUser, Root,
};
use pubky_testnet::Testnet;
use serde_json::Value;
use tempfile::TempDir;
use tokio::sync::watch;

use crate::migrator::{self, Report};

pub struct SpecMigrationTest {
    /// Keeps the homeservers running.
    testnet: Testnet,
    /// Bulk-indexed by nexus.
    pub primary: PubkyId,
    /// Read per user.
    pub external: PubkyId,
    bulk: HsEventProcessorRunner,
    client: Arc<dyn HomeserverClient>,
    handler: Arc<dyn EventHandler>,
    ingestor: Arc<UserIngestor>,
    retry_queue: Arc<RetryQueue>,
    retry: Arc<RetryScheduler>,
    not_found_backoff: Arc<UserNotFoundBackoff>,
    shutdown_rx: watch::Receiver<bool>,
    _shutdown_tx: watch::Sender<bool>,
    files: TempDir,
}

impl SpecMigrationTest {
    pub async fn setup() -> Result<Self> {
        StackManager::setup(&StackConfig::default())
            .await
            .map_err(|e| anyhow!("stack setup failed: {e:?}"))?;
        let files = TempDir::new()?;

        let mut testnet = Testnet::new_unseeded().await?;
        testnet.create_http_relay().await?;
        let primary = PubkyId::from(testnet.create_random_homeserver().await?.public_key());
        let external = PubkyId::from(testnet.create_random_homeserver().await?.public_key());
        Homeserver::persist_if_unknown(primary.clone())
            .await
            .map_err(|e| anyhow!("persisting the primary: {e}"))?;

        // Nexus's own client: one per process, bound to this testnet
        PubkyConnector::init_from(testnet.sdk()?)
            .await
            .map_err(|e| anyhow!("pubky connector: {e}"))?;
        let pubky = PubkyConnector::get().map_err(|e| anyhow!("pubky connector: {e}"))?;
        let client: Arc<dyn HomeserverClient> =
            Arc::new(PubkyHomeserverClient::new(pubky, DEFAULT_MAX_FILE_SIZE));

        let moderation = Arc::new(Moderation {
            id: PubkyId::from(Keypair::random().public_key()),
            tags: Vec::new(),
        });
        let ingestor = default_ingestor_tests();
        // One retry queue: the scheduler enqueues to it and the handler's gate reads it
        let retry_queue = Arc::new(RetryQueue::default());
        let store: Arc<dyn RetryStore> = retry_queue.clone();
        let handler: Arc<dyn EventHandler> = Arc::new(DefaultEventHandler::new(
            moderation,
            ingestor.clone(),
            client.clone(),
            files.path().to_path_buf(),
            store.clone(),
        ));
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
            client: client.clone(),
            shutdown_rx: shutdown_rx.clone(),
            primary_homeserver: primary.clone(),
            retry_scheduler: retry.clone(),
        };

        Ok(Self {
            testnet,
            primary,
            external,
            bulk,
            client,
            handler,
            ingestor,
            retry_queue,
            retry,
            not_found_backoff: Arc::new(UserNotFoundBackoff::default()),
            shutdown_rx,
            _shutdown_tx: shutdown_tx,
            files,
        })
    }

    // ----- nexus, one step at a time -----

    /// One poll of the primary's stream, then one per-user read of the external homeserver:
    /// everything written so far, read once.
    pub async fn index(&self) -> Result<()> {
        self.bulk
            .build(self.primary.as_ref())
            .await
            .map_err(|e| anyhow!("{e}"))?
            .run()
            .await
            .map_err(|e| anyhow!("{e:?}"))?;
        Arc::new(KeyBasedEventProcessor {
            homeserver_id: self.external.clone(),
            limit: 100,
            event_handler: self.handler.clone(),
            client: self.client.clone(),
            user_not_found_backoff: self.not_found_backoff.clone(),
            hs_blacklist: HsBlacklist::default(),
            retry_scheduler: self.retry.clone(),
            shutdown_rx: self.shutdown_rx.clone(),
            // One read at a time, each a full pass: nothing to resume
            user_resume_points: Arc::new(UserResumePoints::default()),
            budget: KEY_BASED_RUN_BUDGET,
        })
        .run()
        .await
        .map_err(|e| anyhow!("{e:?}"))
    }

    /// One pass of the retry processor over everything queued so far, as if every entry's
    /// backoff had passed.
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
        .map_err(|e| anyhow!("{e:?}"))
    }

    /// The event URIs queued for retry.
    pub async fn queued(&self) -> Result<Vec<String>> {
        let mut uris: Vec<String> = self
            .retry_queue
            .entries
            .fetch_ready(i64::MAX, None)
            .await
            .map_err(|e| anyhow!("{e}"))?
            .into_iter()
            .map(|(_, event)| event.event_uri)
            .collect();
        uris.sort();
        Ok(uris)
    }

    /// The bytes of the `main` nexus serves for `owner`'s `social/v1` file `{hash}.{ext}`, if it
    /// holds one.
    pub fn served_file(&self, owner: &User, hash: &str, ext: &str) -> Option<Vec<u8>> {
        let folder = file_folder(&owner.id(), &format!("{hash}.{ext}"));
        std::fs::read(self.files.path().join(folder).join("main")).ok()
    }

    // ----- users -----

    /// A user signed up on `hs` with a `pubky.app` profile named `name`. Nexus meets a user of
    /// the external homeserver the way it meets an unknown one, through the ingestor.
    pub async fn user(&self, name: &str, hs: &PubkyId) -> Result<User> {
        let kp = Keypair::random();
        let device = self.testnet.sdk()?;
        let signer = device.signer(kp.clone());
        let hs_key = hs
            .to_public_key()
            .map_err(|e| anyhow!("homeserver key: {e}"))?;
        signer.signup(&hs_key, None).await?;
        let session = signer.signin(client_id()).await?;
        let user = User {
            id: PubkyId::from(kp.public_key()),
            session,
        };
        user.put_json(&PubkyAppUser::create_path(), &v0_profile(name))
            .await?;
        if hs == &self.external {
            self.ingestor
                .maybe_ingest_user(&user.id)
                .await
                .map_err(|e| anyhow!("ingesting the user: {e}"))?;
        }
        Ok(user)
    }
}

/// Client ID the users sign in with. Since pubky 0.10 a session is a grant issued to a client.
fn client_id() -> ClientId {
    ClientId::new("nexus-watcher.spec-migration").expect("static client id is valid")
}

pub fn v0_profile(name: &str) -> PubkyAppUser {
    PubkyAppUser {
        name: name.to_string(),
        bio: Some("spec migration suite".to_string()),
        image: None,
        links: None,
        status: None,
    }
}

/// A user and their session on their homeserver.
pub struct User {
    pub id: PubkyId,
    pub session: PubkySession,
}

impl User {
    pub fn id(&self) -> String {
        self.id.to_string()
    }

    // ----- the pubky.app app -----

    /// Writes a `pubky.app` post. Returns its id.
    pub async fn post(&self, post: &PubkyAppPost) -> Result<String> {
        let id = post.create_id();
        self.put_json(&PubkyAppPost::create_path(&id), post).await?;
        // Post ids are timestamps: keep consecutive posts distinct
        tokio::time::sleep(Duration::from_millis(5)).await;
        Ok(id)
    }

    pub async fn note(&self, text: &str) -> Result<String> {
        self.post(&v0_post(text, None, None, None)).await
    }

    pub async fn reply(&self, text: &str, parent: &str) -> Result<String> {
        self.post(&v0_post(text, Some(parent), None, None)).await
    }

    pub async fn repost(&self, text: &str, target: &str) -> Result<String> {
        self.post(&v0_post(text, None, Some(target), None)).await
    }

    /// Uploads `bytes` as a blob and writes the `pubky.app` File object naming it. Returns the
    /// File object's address and the blob's hash.
    pub async fn upload(
        &self,
        name: &str,
        content_type: &str,
        bytes: &[u8],
    ) -> Result<(String, String)> {
        let blob = PubkyAppBlob::new(bytes.to_vec());
        let hash = blob.create_id();
        self.session
            .storage()
            .put(PubkyAppBlob::create_path(&hash), blob.0)
            .await?;
        let file = PubkyAppFile {
            name: name.to_string(),
            content_type: content_type.to_string(),
            src: legacy_v0::blob_uri_builder(self.id(), hash.clone()),
            size: bytes.len(),
            created_at: Utc::now().timestamp_millis(),
        };
        let file_id = file.create_id();
        self.put_json(&PubkyAppFile::create_path(&file_id), &file)
            .await?;
        tokio::time::sleep(Duration::from_millis(5)).await;
        Ok((legacy_v0::file_uri_builder(self.id(), file_id), hash))
    }

    pub async fn follow(&self, followee: &User) -> Result<()> {
        let follow = PubkyAppFollow {
            created_at: Utc::now().timestamp_millis(),
        };
        self.put_json(&PubkyAppFollow::create_path(&followee.id()), &follow)
            .await
    }

    /// Tags `target`, a `pubky://` address.
    pub async fn tag(&self, target: &str, label: &str) -> Result<()> {
        let tag = PubkyAppTag {
            uri: target.to_string(),
            label: label.to_string(),
            created_at: Utc::now().timestamp_millis(),
        };
        self.put_json(&PubkyAppTag::create_path(&tag.create_id()), &tag)
            .await
    }

    pub async fn bookmark(&self, target: &str) -> Result<()> {
        let bookmark = PubkyAppBookmark {
            uri: target.to_string(),
            created_at: Utc::now().timestamp_millis(),
        };
        self.put_json(
            &PubkyAppBookmark::create_path(&bookmark.create_id()),
            &bookmark,
        )
        .await
    }

    // ----- the migration -----

    /// The app's migration of this user's tree, whole.
    pub async fn migrate(&self) -> Result<Report> {
        migrator::migrate(&self.session, &self.id).await
    }

    /// The app's migration, started: the trees are listed, nothing is copied yet.
    pub async fn start_migration(&self) -> Result<migrator::Migration<'_>> {
        migrator::Migration::start(&self.session, &self.id).await
    }

    // ----- the social/v1 app -----

    /// Writes a new `social/v1` post. Returns its id.
    pub async fn v1_post(&self, post: &PubkySocialPost) -> Result<String> {
        let minted = post
            .create_version(Root::Pub, &self.id, None)
            .map_err(|e| anyhow!("minting a post: {e}"))?;
        self.put_json(&minted.path, post).await?;
        tokio::time::sleep(Duration::from_millis(5)).await;
        Ok(minted.id)
    }

    pub async fn v1_reply(&self, text: &str, parent: &str) -> Result<String> {
        self.v1_post(&v1_note(text, Some(parent))).await
    }

    pub async fn v1_follow(&self, followee: &User) -> Result<()> {
        self.put_json(
            &PubkySocialFollow::create_path(&followee.id()),
            &PubkySocialFollow::new(),
        )
        .await
    }

    /// Tags `target` from the `social/v1` app.
    pub async fn v1_tag(&self, target: &str, label: &str) -> Result<()> {
        let tag = PubkySocialTag::new(target.to_string(), label.to_string());
        self.put_json(&PubkySocialTag::create_path(&tag.create_id()), &tag)
            .await
    }

    pub async fn v1_profile(&self, name: &str, image: Option<String>) -> Result<()> {
        let profile = PubkySocialUser::new(
            name.to_string(),
            Some("spec migration suite".to_string()),
            image,
            None,
            None,
        );
        self.put_json(&PubkySocialUser::create_path(), &profile)
            .await
    }

    // ----- storage -----

    pub async fn put_json<T: serde::Serialize>(&self, path: &str, object: &T) -> Result<()> {
        self.session
            .storage()
            .put(path, serde_json::to_vec(object)?)
            .await
            .with_context(|| format!("PUT {path}"))?;
        Ok(())
    }
}

pub fn v0_post(
    text: &str,
    parent: Option<&str>,
    repost_of: Option<&str>,
    attachments: Option<Vec<String>>,
) -> PubkyAppPost {
    PubkyAppPost {
        content: text.to_string(),
        kind: PubkyAppPostKind::Short,
        parent: parent.map(str::to_string),
        embed: repost_of.map(|uri| PubkyAppPostEmbed {
            kind: PubkyAppPostKind::Short,
            uri: uri.to_string(),
        }),
        attachments,
        lock: None,
    }
}

pub fn v1_note(text: &str, parent: Option<&str>) -> PubkySocialPost {
    PubkySocialPost::new(
        text.to_string(),
        PubkySocialPostKind::Note,
        parent.map(str::to_string),
        None,
        vec![],
    )
}

// ----- addresses -----

pub fn v0_post_uri(author: &User, post_id: &str) -> String {
    legacy_v0::post_uri_builder(author.id(), post_id.to_string())
}

/// The owner-relative path of a `pubky.app` post.
pub fn v0_post_path(post_id: &str) -> String {
    PubkyAppPost::create_path(post_id)
}

pub fn v1_post_uri(author: &User, post_id: &str) -> String {
    pubky_social_specs::post_uri_builder(author.id(), post_id.to_string())
}

pub fn v0_profile_uri(user: &User) -> String {
    legacy_v0::user_uri_builder(user.id())
}

pub fn v1_profile_uri(user: &User) -> String {
    pubky_social_specs::user_uri_builder(user.id())
}

pub fn v1_file_uri(owner: &User, hash: &str, ext: &str) -> String {
    pubky_social_specs::file_uri_builder(owner.id(), format!("{hash}.{ext}"))
}

// ----- what nexus serves -----

pub async fn profile(user: &User) -> Result<UserView> {
    UserView::get_by_id(&user.id(), None, None)
        .await
        .map_err(|e| anyhow!("{e}"))?
        .ok_or_else(|| anyhow!("{} is not indexed", user.id()))
}

pub async fn epoch(user: &User) -> Result<Epoch> {
    Ok(profile(user).await?.details.epoch)
}

/// The post as served, with `viewer`'s bookmark and tag relationship.
pub async fn post(author: &User, post_id: &str, viewer: Option<&User>) -> Result<Option<PostView>> {
    let viewer = viewer.map(User::id);
    PostView::get_by_id(&author.id(), post_id, viewer.as_deref(), None, None)
        .await
        .map_err(|e| anyhow!("{e}"))
}

/// Whether `viewer` has bookmarked `author`'s post, as the post is served to them.
pub async fn has_bookmarked(viewer: &User, author: &User, post_id: &str) -> Result<bool> {
    let view = post(author, post_id, Some(viewer))
        .await?
        .ok_or_else(|| anyhow!("post {}:{post_id} is not served", author.id()))?;
    Ok(view.bookmark.is_some())
}

pub async fn served_post(author: &User, post_id: &str) -> Result<PostView> {
    post(author, post_id, None)
        .await?
        .ok_or_else(|| anyhow!("post {}:{post_id} is not served", author.id()))
}

/// The ids of the author's root posts, newest first.
pub async fn author_posts(author: &User) -> Result<Vec<String>> {
    let stream = PostStream::get_author_posts(
        &author.id(),
        Default::default(),
        None,
        None,
        None,
        Some(100),
        false,
    )
    .await
    .map_err(|e| anyhow!("{e}"))?;
    Ok(stream
        .post_keys
        .into_iter()
        .map(|key| {
            key.split_once(':')
                .map(|(_, id)| id.to_string())
                .unwrap_or(key)
        })
        .collect())
}

pub async fn followers(user: &User) -> Result<Vec<String>> {
    Ok(Followers::get_by_id(&user.id(), None, Some(100))
        .await
        .map_err(|e| anyhow!("{e}"))?
        .map(|f| f.0)
        .unwrap_or_default())
}

pub async fn following(user: &User) -> Result<Vec<String>> {
    Ok(Following::get_by_id(&user.id(), None, Some(100))
        .await
        .map_err(|e| anyhow!("{e}"))?
        .map(|f| f.0)
        .unwrap_or_default())
}

/// `user`'s notifications as `(time, body)`, newest first, the body whole: its kind, actor,
/// addresses and label. Identical notifications share one entry. A follow or a tag issued again
/// moves that entry's time to the latest; a reply, repost or mention issued again keeps its
/// first time, so it shows no change at all.
pub async fn notifications(user: &User) -> Result<Vec<(i64, Value)>> {
    let list = Notification::get_by_id(&user.id(), Pagination::default())
        .await
        .map_err(|e| anyhow!("{e}"))?;
    list.into_iter()
        .map(|n| Ok((n.timestamp, serde_json::to_value(&n.body)?)))
        .collect()
}

/// `(label, taggers)` of the tags on a post, sorted.
pub async fn post_tags(author: &User, post_id: &str) -> Result<Vec<(String, Vec<String>)>> {
    Ok(tag_list(served_post(author, post_id).await?.tags))
}

/// `(label, taggers)` of the tags on a user's profile, sorted.
pub async fn profile_tags(user: &User) -> Result<Vec<(String, Vec<String>)>> {
    Ok(tag_list(profile(user).await?.tags))
}

fn tag_list(tags: Vec<TagDetails>) -> Vec<(String, Vec<String>)> {
    let mut list: Vec<_> = tags
        .into_iter()
        .map(|tag| {
            let mut taggers = tag.taggers;
            taggers.sort();
            (tag.label, taggers)
        })
        .collect();
    list.sort();
    list
}

/// The in-memory retry queue, with a switch that makes the next fetch take every entry.
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

    /// The first fetch after [`SpecMigrationTest::retry`] takes every entry; the processor's
    /// next fetch sees only what is really due, which ends the pass.
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
