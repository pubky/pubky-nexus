use anyhow::{anyhow, Error, Result};
use chrono::Utc;
use nexus_common::db::PubkyClient;
use nexus_common::get_files_dir_pathbuf;
use nexus_common::get_files_dir_test_pathbuf;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use nexus_watcher::events::retry::event::RetryEvent;
use nexus_watcher::events::Event;
use nexus_watcher::events::Moderation;
use nexus_watcher::service::EventProcessorFactory;
use nexus_watcher::service::NexusWatcher;
use nexus_watcher::service::TEventProcessorFactory;
use pubky::Keypair;
use pubky::PublicKey;
use pubky_app_specs::PubkyId;
use pubky_app_specs::{
    traits::TimestampId, PubkyAppFile, PubkyAppFollow, PubkyAppPost, PubkyAppUser,
};
use pubky_testnet::EphemeralTestnet;
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;

use crate::event_processor::utils::default_moderation_tests;

/// Struct to hold the setup environment for tests
pub struct WatcherTest {
    /// We keep the testnet instance to prevent it from being dropped while the tests are running
    /// If you drop the testnet, the watcher will not be able to connect to the homeserver
    #[allow(unused)]
    pub testnet: EphemeralTestnet,
    /// The homeserver ID
    pub homeserver_id: String,
    /// The event processor factory
    pub event_processor_factory: EventProcessorFactory,
    /// Whether to ensure event processing is complete
    pub ensure_event_processing: bool,
}

impl WatcherTest {
    /// Creates a test event processor factory with predefined configuration.
    ///
    /// This function sets up an `EventProcessorFactory` specifically for testing environments
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
    /// Returns a fully configured `EventProcessorFactory` ready for use in tests.
    fn create_test_event_processor_factory() -> EventProcessorFactory {
        let moderation = Arc::new(default_moderation_tests());

        let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        EventProcessorFactory {
            limit: 1000,
            files_path: get_files_dir_test_pathbuf(),
            tracer_name: String::from("watcher.test"),
            moderation,
            shutdown_rx,
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
        let mut testnet = EphemeralTestnet::start_minimal().await?;

        // Create a random homeserver with a random public key
        let homeserver_id = testnet
            .create_random_homeserver()
            .await?
            .public_key()
            .to_string();
        let pubky_id = PubkyId::try_from(&homeserver_id).unwrap();
        Homeserver::persist_if_unknown(pubky_id.clone())
            .await
            .unwrap();

        // Initialize the PubkyConnector with the test homeserver client
        let client = testnet.pubky_client_builder().build().unwrap();
        match PubkyClient::init_from_client(client).await {
            Ok(_) => debug!("WatcherTest: PubkyConnector initialised"),
            Err(e) => debug!("WatcherTest: {}", e),
        }

        // Initialize the test-scoped EventProcessorFactory; mirrors the standard processor behavior
        let event_processor_factory = Self::create_test_event_processor_factory();

        Ok(Self {
            testnet,
            homeserver_id,
            event_processor_factory,
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
            self.event_processor_factory
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
    /// - `homeserver_uri`: The URI of the homeserver to write the data to.
    /// - `object`: A generic type representing the data to be sent, which must implement `serde::Serialize`.
    pub async fn put<T>(&mut self, homeserver_uri: &str, object: T) -> Result<()>
    where
        T: serde::Serialize,
    {
        let pubky_client = PubkyClient::get().unwrap();
        pubky_client
            .put(homeserver_uri)
            .json(&object)
            .send()
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
    /// - `homeserver_uri`: The URI of the homeserver from which content should be deleted.
    ///
    pub async fn del(&mut self, homeserver_uri: &str) -> Result<()> {
        let pubky_client = PubkyClient::get().unwrap();
        pubky_client.delete(homeserver_uri).send().await?;
        self.ensure_event_processing_complete().await?;
        Ok(())
    }

    /// Registers a user in the homeserver with the keypair.
    /// # Arguments
    /// * `keypair` - A reference to the `Keypair` used for signing up the user.
    pub async fn register_user(&self, keypair: &Keypair) -> Result<()> {
        let pubky_client = PubkyClient::get().unwrap();

        let public_key: PublicKey = self.homeserver_id.clone().try_into().unwrap();
        pubky_client.signup(keypair, &public_key, None).await?;
        Ok(())
    }

    pub async fn create_user(&mut self, keypair: &Keypair, user: &PubkyAppUser) -> Result<String> {
        let user_id = keypair.public_key().to_z32();
        let pubky_client = PubkyClient::get().unwrap();
        let public_key: PublicKey = self.homeserver_id.clone().try_into().unwrap();
        // Register the key in the homeserver
        pubky_client.signup(keypair, &public_key, None).await?;
        let url = format!("pubky://{user_id}/pub/pubky.app/profile.json");

        // Write the user profile in the pubky.app repository
        pubky_client.put(url.as_str()).json(&user).send().await?;

        // Index to Nexus from Homeserver using the events processor
        self.ensure_event_processing_complete().await?;
        Ok(user_id)
    }

    /// If we attempt two consecutive sign-ups with the same key, the homeserver returns the following error:
    /// 412 Precondition Failed - Compare and swap failed; there is a more recent SignedPacket than the one seen before publishing.
    /// To prevent this error after the first sign-up, we will create/update the existing record instead of creating a new one
    pub async fn create_profile(&mut self, user_id: &str, user: &PubkyAppUser) -> Result<String> {
        let pubky_client = PubkyClient::get().unwrap();
        let url = format!("pubky://{user_id}/pub/pubky.app/profile.json");

        // Write the user profile in the pubky.app repository
        pubky_client.put(url.as_str()).json(&user).send().await?;

        // Index to Nexus from Homeserver using the events processor
        self.ensure_event_processing_complete().await?;
        Ok(user_id.to_string())
    }

    pub async fn create_post(&mut self, user_id: &str, post: &PubkyAppPost) -> Result<String> {
        let post_id = post.create_id();
        let url = format!("pubky://{user_id}/pub/pubky.app/posts/{post_id}");
        // Write the post in the pubky.app repository
        PubkyClient::get()
            .unwrap()
            .put(url.as_str())
            .json(&post)
            .send()
            .await?;

        // Index to Nexus from Homeserver using the events processor
        self.ensure_event_processing_complete().await?;
        Ok(post_id)
    }

    pub async fn cleanup_user(&mut self, user_id: &str) -> Result<()> {
        let url = format!("pubky://{user_id}/pub/pubky.app/profile.json");
        PubkyClient::get()
            .unwrap()
            .delete(url.as_str())
            .send()
            .await?;
        self.ensure_event_processing_complete().await?;
        Ok(())
    }

    pub async fn cleanup_post(&mut self, user_id: &str, post_id: &str) -> Result<()> {
        let url = format!("pubky://{user_id}/pub/pubky.app/posts/{post_id}");
        PubkyClient::get()
            .unwrap()
            .delete(url.as_str())
            .send()
            .await?;
        self.ensure_event_processing_complete().await?;
        Ok(())
    }

    pub async fn create_file(
        &mut self,
        user_id: &str,
        file: &PubkyAppFile,
    ) -> Result<(String, String)> {
        let file_id = file.create_id();
        let url = format!("pubky://{user_id}/pub/pubky.app/files/{file_id}");
        PubkyClient::get()
            .unwrap()
            .put(url.as_str())
            .json(&file)
            .send()
            .await?;

        self.ensure_event_processing_complete().await?;
        Ok((file_id, url))
    }

    pub async fn create_file_from_body(
        &mut self,
        homeserver_uri: &str,
        object: Vec<u8>,
    ) -> Result<()> {
        PubkyClient::get()
            .unwrap()
            .put(homeserver_uri)
            .body(object)
            .send()
            .await?;
        Ok(())
    }

    pub async fn cleanup_file(&mut self, user_id: &str, file_id: &str) -> Result<()> {
        let url = format!("pubky://{user_id}/pub/pubky.app/files/{file_id}");
        PubkyClient::get()
            .unwrap()
            .delete(url.as_str())
            .send()
            .await?;
        self.ensure_event_processing_complete().await?;
        Ok(())
    }

    pub async fn create_follow(&mut self, follower_id: &str, followee_id: &str) -> Result<String> {
        let follow_relationship = PubkyAppFollow {
            created_at: Utc::now().timestamp_millis(),
        };
        let follow_url = format!("pubky://{follower_id}/pub/pubky.app/follows/{followee_id}");
        PubkyClient::get()
            .unwrap()
            .put(follow_url.as_str())
            .json(&follow_relationship)
            .send()
            .await?;
        // Process the event
        self.ensure_event_processing_complete().await?;
        Ok(follow_url)
    }

    pub async fn create_mute(&mut self, muter_id: &str, mutee_id: &str) -> Result<String> {
        let mute_relationship = PubkyAppFollow {
            created_at: Utc::now().timestamp_millis(),
        };
        let mute_url = format!("pubky://{muter_id}/pub/pubky.app/mutes/{mutee_id}");
        PubkyClient::get()
            .unwrap()
            .put(mute_url.as_str())
            .json(&mute_relationship)
            .send()
            .await?;
        // Process the event
        self.ensure_event_processing_complete().await?;
        Ok(mute_url)
    }
}

/// Retrieves an event from the homeserver and handles it asynchronously.
/// # Arguments
/// * `event_line` - A string slice that represents the URI of the event to be retrieved
///   from the homeserver. It contains the event type and the homeserver uri
pub async fn retrieve_and_handle_event_line(
    event_line: &str,
    moderation: Arc<Moderation>,
) -> Result<(), DynError> {
    let event = Event::parse_event(event_line, get_files_dir_pathbuf()).unwrap_or_default();

    if let Some(event) = event {
        event.clone().handle(moderation).await?
    }

    Ok(())
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
