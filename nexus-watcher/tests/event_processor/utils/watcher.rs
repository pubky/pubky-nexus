use anyhow::{anyhow, Error, Result};
use base32::{encode, Alphabet};
use chrono::Utc;
use nexus_common::db::PubkyConnector;
use nexus_common::get_files_dir_pathbuf;
use nexus_common::get_files_dir_test_pathbuf;
use nexus_common::models::event::Event;
use nexus_common::models::file::FileDetails;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::models::traits::Collection;
use nexus_common::types::DynError;
use nexus_watcher::events::retry::event::RetryEvent;
use nexus_watcher::events::{handle, Moderation};
use nexus_watcher::service::EventProcessorRunner;
use nexus_watcher::service::NexusWatcher;
use nexus_watcher::service::TEventProcessorRunner;
use pubky::Keypair;
use pubky::PublicKey;
use pubky::ResourcePath;
use pubky_app_specs::file_uri_builder;
use pubky_app_specs::traits::HashId;
use pubky_app_specs::{
    traits::{HasIdPath, HasPath, TimestampId},
    PubkyAppFile, PubkyAppFollow, PubkyAppMute, PubkyAppPost, PubkyAppUser, PubkyId,
};
use pubky_testnet::EphemeralTestnet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;

use crate::event_processor::utils::default_moderation_tests;

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a unique post ID for tests.
/// Uses PID-based offset for inter-process uniqueness and atomic counter
/// for intra-process uniqueness.
pub fn generate_post_id() -> String {
    let now = Utc::now().timestamp_micros() as u64;
    let pid_offset = (std::process::id() as u64) * 1000;
    let count = COUNTER.fetch_add(1, Ordering::SeqCst);

    let timestamp = now + pid_offset + count;

    let bytes = timestamp.to_be_bytes();
    encode(Alphabet::Crockford, &bytes)
}

/// Struct to hold the setup environment for tests
pub struct WatcherTest {
    pub testnet: EphemeralTestnet,
    /// The homeserver ID
    pub homeserver_id: String,
    /// The event processor runner
    pub event_processor_runner: EventProcessorRunner,
    /// Whether to ensure event processing is complete
    pub ensure_event_processing: bool,
}

impl WatcherTest {
    /// Creates a test event processor runner with predefined configuration.
    ///
    /// This function sets up an `EventProcessorRunner` specifically for testing environments
    /// with hardcoded values that are appropriate for test scenarios.
    ///
    /// # Configuration Details
    /// - **Limit**: Set to 1000 events for test performance
    /// - **Files Path**: Uses test directory path for file operations
    /// - **Tracer Name**: Uses "watcher.test" for test-specific logging
    /// - **Moderation**: Configured with hardcoded moderator key and test tags
    ///
    /// # Moderation Setup
    /// Uses a hardcoded moderator public key and test moderation tags ("label_to_moderate")
    /// that are designed specifically for test scenarios and should not be used in production.
    ///
    /// # Returns
    /// Returns a fully configured `EventProcessorRunner` ready for use in tests.
    fn create_test_event_processor_runner(default_homeserver: PubkyId) -> EventProcessorRunner {
        let moderation = Arc::new(default_moderation_tests());

        let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        EventProcessorRunner {
            limit: 1000,
            monitored_homeservers_limit: 100,
            files_path: get_files_dir_test_pathbuf(),
            tracer_name: String::from("watcher.test"),
            moderation,
            shutdown_rx,
            default_homeserver,
        }
    }

    /// Sets up the test environment for the watcher.
    ///
    /// This function performs the following steps:
    /// 1. Reads configuration from environment variables.
    /// 2. Initializes database connectors for Neo4j and Redis.
    /// 3. Sets up the global DHT test network for the watcher (ephemeral testnet).
    /// 4. Creates and starts a test homeserver instance with a random public key.
    /// 5. Initializes the PubkyConnector with the test homeserver client.
    /// 6. Creates and configures the event processor with the test homeserver URL.
    /// 7. Creates a channel to signal the event processor to shutdown.
    ///
    /// # Returns
    /// Returns an instance of `Self` containing the configuration, homeserver,
    /// event processor, and other test setup details, including the shutdown receiver.
    pub async fn setup() -> Result<Self> {
        if let Err(e) = NexusWatcher::builder().init_test_stack().await {
            return Err(Error::msg(format!("could not initialise the stack, {e:?}")));
        }

        // WARNING: testnet initialization is time expensive, we only init one per process
        // TODO: Maybe we should create a single testnet network (singleton and push there more homeservers)
        let mut testnet = EphemeralTestnet::builder()
            .with_http_relay()
            .build()
            .await?;

        // Create a random homeserver with a random public key
        let homeserver_id = testnet.create_random_homeserver().await?.public_key().z32();
        let pubky_id = PubkyId::try_from(&homeserver_id).unwrap();
        Homeserver::persist_if_unknown(pubky_id.clone())
            .await
            .unwrap();

        // Initialize the PubkyConnector with the test homeserver client
        let sdk = testnet.sdk().unwrap();
        match PubkyConnector::init_from(sdk).await {
            Ok(_) => debug!("WatcherTest: PubkyConnector initialised"),
            Err(e) => panic!("WatcherTest: PubkyConnector initialization failed: {}", e),
        }

        // Initialize the test-scoped EventProcessorRunner; mirrors the standard processor behavior
        let event_processor_runner = Self::create_test_event_processor_runner(pubky_id);

        Ok(Self {
            testnet,
            homeserver_id,
            event_processor_runner,
            ensure_event_processing: true,
        })
    }

    /// Disables event processing and returns the modified instance.
    pub async fn remove_event_processing(mut self) -> Self {
        self.ensure_event_processing = false;
        self
    }

    /// Ensures that event processing is completed if it is enabled.
    pub async fn ensure_event_processing_complete(&mut self) -> Result<()> {
        if self.ensure_event_processing {
            self.event_processor_runner
                .build(self.homeserver_id.clone())
                .await
                .map_err(|e| anyhow!(e))?
                .run()
                .await
                .map_err(|e| anyhow!(e))?;
        }
        Ok(())
    }

    /// Sends a PUT request to the homeserver with the provided object of data.
    ///
    /// This function performs the following steps:
    /// 1. Retrieves the Pubky client from the PubkyConnector.
    /// 2. Sends the object data to the specified homeserver URI using a PUT request.
    /// 3. Ensures that all event processing is complete after the PUT operation.
    ///
    /// # Parameters
    /// - `hs_path`: The homeserver path to the file to write the data to.
    /// - `object`: A generic type representing the data to be sent, which must implement `serde::Serialize`.
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

    /// Sends a DELETE request to the homeserver to remove content.
    ///
    /// This function performs the following steps:
    /// 1. Retrieves the Pubky client from the PubkyConnector.
    /// 2. Sends a DELETE request to the specified homeserver URI.
    /// 3. Ensures that all event processing is complete after the DELETE operation.
    ///
    /// # Parameters
    /// - `hs_path`: The homeserver path to the file to be deleted.
    ///
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
        let hs_pk: PublicKey = self.homeserver_id.clone().try_into()?;
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
        // Register the key in the homeserver
        self.register_user(user_kp).await?;

        // Write the user profile in the pubky.app repository
        let user_path = PubkyAppUser::hs_path();
        self.put(user_kp, &user_path, &user).await?;

        // Index to Nexus from Homeserver using the events processor
        self.ensure_event_processing_complete().await?;
        Ok(user_id)
    }

    /// If we attempt two consecutive sign-ups with the same key, the homeserver returns the following error:
    /// 412 Precondition Failed - Compare and swap failed; there is a more recent SignedPacket than the one seen before publishing.
    /// To prevent this error after the first sign-up, we will create/update the existing record instead of creating a new one
    pub async fn create_profile(
        &mut self,
        user_kp: &Keypair,
        user: &PubkyAppUser,
    ) -> Result<String> {
        let user_id = user_kp.public_key().to_z32();

        // Write the user profile in the pubky.app repository
        let user_path = PubkyAppUser::hs_path();
        self.put(user_kp, &user_path, &user).await?;

        // Index to Nexus from Homeserver using the events processor
        self.ensure_event_processing_complete().await?;
        Ok(user_id.to_string())
    }

    /// Creates a post with a unique ID generated from the current timestamp.
    /// Uses atomic counter and PID offset for uniqueness across parallel test runs.
    pub async fn create_post(
        &mut self,
        user_kp: &Keypair,
        post: &PubkyAppPost,
    ) -> Result<(String, ResourcePath)> {
        let post_id = generate_post_id();
        let post_path: ResourcePath = PubkyAppPost::create_path(&post_id).parse()?;
        // Write the post in the pubky.app repository
        self.put(user_kp, &post_path, post).await?;

        // Index to Nexus from Homeserver using the events processor
        self.ensure_event_processing_complete().await?;
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

        self.ensure_event_processing_complete().await?;
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
        // Process the event
        self.ensure_event_processing_complete().await?;
        Ok(follow_path)
    }

    pub async fn create_mute(
        &mut self,
        muter_kp: &Keypair,
        mutee_id: &str,
    ) -> Result<ResourcePath> {
        let mute_relationship = PubkyAppMute {
            created_at: Utc::now().timestamp_millis(),
        };
        let mute_path = PubkyAppMute::hs_path(mutee_id);
        self.put(muter_kp, &mute_path, mute_relationship).await?;
        // Process the event
        self.ensure_event_processing_complete().await?;
        Ok(mute_path)
    }
}

/// Retrieves an event from the homeserver and handles it asynchronously.
/// # Arguments
/// * `event_line` - A string slice that represents the URI of the event to be retrieved
///   from the homeserver. It contains the event type and the homeserver uri
///
/// # Errors
/// Throws an error if event parsing fails
pub async fn retrieve_and_handle_event_line(
    event_line: &str,
    moderation: Arc<Moderation>,
) -> Result<(), DynError> {
    match Event::parse_event(event_line, get_files_dir_pathbuf())? {
        Some(event) => handle(&event, moderation).await,
        None => Ok(()),
    }
}

/// NOTE: This might not be needed anymore because the `RetryManager` runs in the same thread as the watcher
/// Previously, we were spawning the `RetryManager` in a separate task
///
/// Attempts to read an event index with retries before timing out
/// # Arguments
/// * `event_index` - A string slice representing the index to check
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
        // Nap time
        tokio::time::sleep(Duration::from_millis(SLEEP_MS)).await;
    }
    panic!("TIMEOUT: It takes to much time to read the RetryManager new index")
}

/// Common assertions for FileDetails of an existing file
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
