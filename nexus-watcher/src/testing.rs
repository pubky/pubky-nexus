use anyhow::{anyhow, Error, Result};
use base32::{encode, Alphabet};
use chrono::Utc;
use nexus_common::db::PubkyConnector;
use nexus_common::get_files_dir_pathbuf;
use nexus_common::models::event::{Event, EventProcessorError, ParseResult};
use nexus_common::models::file::FileDetails;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::models::traits::Collection;
use nexus_common::plugin::NexusPlugin;
use nexus_common::{StackConfig, StackManager};
use pubky::{Keypair, PublicKey, ResourcePath};
use pubky_app_specs::file_uri_builder;
use pubky_app_specs::traits::HashId;
use pubky_app_specs::{
    traits::{HasIdPath, HasPath, TimestampId},
    PubkyAppFile, PubkyAppFollow, PubkyAppPost, PubkyAppUser, PubkyId,
};
use pubky_testnet::Testnet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tracing::debug;

use crate::dispatcher::EventDispatcher;
use crate::events::retry::event::RetryEvent;
use crate::events::{handle, Moderation};
use crate::service::{EventProcessorRunner, TEventProcessorRunner};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Default moderation settings for watcher tests.
pub fn default_moderation_tests() -> Moderation {
    let id = PubkyId::try_from("uo7jgkykft4885n8cruizwy6khw71mnu5pq3ay9i8pw1ymcn85ko")
        .expect("Hardcoded test moderation key should be valid");
    let tags = vec!["label_to_moderate".to_string()];
    Moderation { id, tags }
}

/// Generate a unique post ID for tests.
pub fn generate_post_id() -> String {
    let now = Utc::now().timestamp_micros() as u64;
    let pid_offset = (std::process::id() as u64) * 1000;
    let count = COUNTER.fetch_add(1, Ordering::SeqCst);

    let timestamp = now + pid_offset + count;

    let bytes = timestamp.to_be_bytes();
    encode(Alphabet::Crockford, &bytes)
}

/// Test harness for watcher and downstream plugin integration tests.
pub struct WatcherTest {
    pub testnet: Testnet,
    pub homeserver_id: PubkyId,
    pub event_processor_runner: EventProcessorRunner,
    pub ensure_event_processing: bool,
    /// Keeps the static files temp dir alive for the test.
    pub temp_dir: TempDir,
}

impl WatcherTest {
    fn create_test_event_processor_runner(
        default_homeserver: PubkyId,
        files_path: PathBuf,
    ) -> EventProcessorRunner {
        let moderation = Arc::new(default_moderation_tests());

        let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        EventProcessorRunner {
            limit: 1000,
            monitored_homeservers_limit: 100,
            files_path,
            tracer_name: "test".to_string(),
            moderation,
            shutdown_rx,
            default_homeserver,
            dispatcher: None,
        }
    }

    /// Sets up the watcher test environment without plugins.
    pub async fn setup() -> Result<Self> {
        Self::setup_with_plugins(vec![]).await
    }

    /// Sets up the watcher test environment with plugins registered in the event dispatcher.
    pub async fn setup_with_plugins(plugins: Vec<Arc<dyn NexusPlugin>>) -> Result<Self> {
        if let Err(e) = StackManager::setup(&StackConfig::default()).await {
            return Err(Error::msg(format!("could not initialise the stack, {e:?}")));
        }

        let temp_dir = TempDir::new()?;
        let files_path = temp_dir.path().to_path_buf();

        let mut testnet = Testnet::new().await?;
        testnet.create_http_relay().await?;

        let homeserver_id = PubkyId::from(testnet.create_random_homeserver().await?.public_key());
        Homeserver::persist_if_unknown(homeserver_id.clone())
            .await
            .unwrap();

        let sdk = testnet.sdk().unwrap();
        match PubkyConnector::init_from(sdk).await {
            Ok(_) => debug!("WatcherTest: PubkyConnector initialised"),
            Err(e) => panic!("WatcherTest: PubkyConnector initialization failed: {}", e),
        }

        let mut event_processor_runner =
            Self::create_test_event_processor_runner(homeserver_id.clone(), files_path);
        if !plugins.is_empty() {
            event_processor_runner.dispatcher = Some(Arc::new(EventDispatcher::new(plugins)));
        }

        Ok(Self {
            testnet,
            homeserver_id,
            event_processor_runner,
            ensure_event_processing: true,
            temp_dir,
        })
    }

    /// Disables automatic event processing after each write.
    pub async fn remove_event_processing(mut self) -> Self {
        self.ensure_event_processing = false;
        self
    }

    pub async fn ensure_event_processing_complete(&mut self) -> Result<()> {
        if self.ensure_event_processing {
            self.event_processor_runner
                .build(self.homeserver_id.to_string())
                .await
                .map_err(|e| anyhow!(e))?
                .run()
                .await
                .map_err(|e| anyhow!(e))?;
        }
        Ok(())
    }

    pub async fn put<T>(
        &mut self,
        user_keypair: &Keypair,
        hs_path: &ResourcePath,
        object: T,
    ) -> Result<()>
    where
        T: serde::Serialize,
    {
        let pubky = PubkyConnector::get()?;

        let signer = pubky.signer(user_keypair.clone());
        let session = signer.signin().await?;
        session
            .storage()
            .put(hs_path, serde_json::to_string(&object)?)
            .await?;
        self.ensure_event_processing_complete().await?;
        Ok(())
    }

    pub async fn del(&mut self, user_keypair: &Keypair, hs_path: &ResourcePath) -> Result<()> {
        let pubky = PubkyConnector::get()?;

        let signer = pubky.signer(user_keypair.clone());
        let session = signer.signin().await?;
        session.storage().delete(hs_path).await?;
        self.ensure_event_processing_complete().await?;
        Ok(())
    }

    pub async fn register_user(&self, user_kp: &Keypair) -> Result<()> {
        let pubky = PubkyConnector::get()?;

        let signer = pubky.signer(user_kp.clone());
        let hs_pk = self.homeserver_id.to_public_key();
        signer.signup(&hs_pk, None).await?;

        Ok(())
    }

    pub async fn register_user_in_hs(&self, user_kp: &Keypair, hs_pk: &PublicKey) -> Result<()> {
        let pubky = PubkyConnector::get()?;

        let signer = pubky.signer(user_kp.clone());
        signer.signup(hs_pk, None).await?;

        Ok(())
    }

    pub async fn create_user(&mut self, user_kp: &Keypair, user: &PubkyAppUser) -> Result<String> {
        let user_id = user_kp.public_key().to_z32();
        self.register_user(user_kp).await?;

        let user_path = PubkyAppUser::hs_path();
        self.put(user_kp, &user_path, user).await?;

        Ok(user_id)
    }

    pub async fn create_profile(
        &mut self,
        user_kp: &Keypair,
        user: &PubkyAppUser,
    ) -> Result<String> {
        let user_id = user_kp.public_key().to_z32();

        let user_path = PubkyAppUser::hs_path();
        self.put(user_kp, &user_path, user).await?;

        Ok(user_id)
    }

    pub async fn create_post(
        &mut self,
        user_kp: &Keypair,
        post: &PubkyAppPost,
    ) -> Result<(String, ResourcePath)> {
        let post_id = generate_post_id();
        let post_path: ResourcePath = PubkyAppPost::create_path(&post_id).parse()?;
        self.put(user_kp, &post_path, post).await?;

        Ok((post_id, post_path))
    }

    pub async fn cleanup_user(&mut self, user_kp: &Keypair) -> Result<()> {
        let user_path = PubkyAppUser::hs_path();
        self.del(user_kp, &user_path).await
    }

    pub async fn cleanup_post(
        &mut self,
        user_kp: &Keypair,
        post_path: &ResourcePath,
    ) -> Result<()> {
        self.del(user_kp, post_path).await
    }

    pub async fn create_file(
        &mut self,
        user_kp: &Keypair,
        file: &PubkyAppFile,
    ) -> Result<(String, ResourcePath)> {
        let file_id = file.create_id();
        let file_path: ResourcePath = PubkyAppFile::create_path(&file_id).parse()?;
        self.put(user_kp, &file_path, file).await?;

        Ok((file_id, file_path))
    }

    pub async fn create_file_from_body(
        &mut self,
        user_kp: &Keypair,
        homeserver_uri: &str,
        object: Vec<u8>,
    ) -> Result<()> {
        let pubky = PubkyConnector::get()?;

        let signer = pubky.signer(user_kp.clone());
        let session = signer.signin().await?;
        session.storage().put(homeserver_uri, object).await?;
        Ok(())
    }

    pub async fn cleanup_file(
        &mut self,
        user_kp: &Keypair,
        file_path: &ResourcePath,
    ) -> Result<()> {
        self.del(user_kp, file_path).await
    }

    pub async fn create_follow(
        &mut self,
        follower_kp: &Keypair,
        followee_id: &str,
    ) -> Result<ResourcePath> {
        let follow_relationship = PubkyAppFollow {
            created_at: Utc::now().timestamp_millis(),
        };
        let follow_path = follow_relationship.hs_path(followee_id);
        self.put(follower_kp, &follow_path, follow_relationship)
            .await?;
        Ok(follow_path)
    }
}

pub async fn retrieve_and_handle_event_line(
    event_line: &str,
    moderation: Arc<Moderation>,
) -> Result<(), EventProcessorError> {
    match Event::parse_event(event_line, get_files_dir_pathbuf())? {
        ParseResult::Parsed(event) => handle(&event, moderation).await,
        ParseResult::Skipped => Ok(()),
        ParseResult::UnrecognizedUri { reason, .. } => Err(EventProcessorError::InvalidEventLine(
            format!("Cannot parse event URI: {reason}"),
        )),
    }
}

pub async fn assert_eventually_exists(event_index: &str) {
    const SLEEP_MS: u64 = 3;
    const MAX_RETRIES: usize = 50;

    for attempt in 0..MAX_RETRIES {
        debug!(
            "RetryEvent: Trying to read index {:?}, attempt {}/{} ({}ms)",
            event_index,
            attempt + 1,
            MAX_RETRIES,
            SLEEP_MS * attempt as u64
        );
        match RetryEvent::check_uri(event_index).await {
            Ok(timeframe) => {
                if timeframe.is_some() {
                    return;
                }
            }
            Err(e) => panic!("Error while getting index: {e:?}"),
        };
        tokio::time::sleep(Duration::from_millis(SLEEP_MS)).await;
    }
    panic!("TIMEOUT: It takes too long to read the RetryManager new index")
}

pub async fn assert_file_details(
    user_id: &str,
    file_id: &str,
    blob_absolute_url: &str,
    file: &PubkyAppFile,
) -> FileDetails {
    let file_absolute_url = file_uri_builder(user_id.into(), file_id.into());

    let files = FileDetails::get_by_ids(vec![vec![user_id, file_id].as_slice()].as_slice())
        .await
        .expect("Failed to fetch files from Nexus");

    let result_file = files[0].as_ref().expect("Created file was not found.");

    assert_eq!(result_file.id, file_id);
    assert_eq!(result_file.src, blob_absolute_url);
    assert_eq!(result_file.uri, file_absolute_url);
    assert_eq!(result_file.size, file.size as i64);
    assert_eq!(result_file.name, file.name);
    assert_eq!(result_file.owner_id, user_id);

    result_file.clone()
}

pub trait HomeserverIdPath: HasIdPath {
    fn hs_path(pubky_id: &str) -> ResourcePath {
        Self::create_path(pubky_id).parse().unwrap()
    }
}
impl<T> HomeserverIdPath for T where T: HasIdPath {}

pub trait HomeserverPath: HasPath {
    fn hs_path() -> ResourcePath {
        Self::create_path().parse().unwrap()
    }
}
impl<T> HomeserverPath for T where T: HasPath {}

pub trait HomeserverHashIdPath: HashId + HasIdPath {
    fn hs_path(&self) -> ResourcePath {
        let id = self.create_id();
        Self::create_path(&id).parse().unwrap()
    }
}
impl<T> HomeserverHashIdPath for T where T: HashId + HasIdPath {}

pub trait HomeserverPathForPubkyId {
    fn hs_path(&self, pubky_id: &str) -> ResourcePath;
}
impl HomeserverPathForPubkyId for PubkyAppFollow {
    fn hs_path(&self, pubky_id: &str) -> ResourcePath {
        Self::create_path(pubky_id).parse().unwrap()
    }
}
