use super::dht::TestnetDHTNetwork;
use anyhow::{anyhow, Result};
use chrono::Utc;
use log::debug;
use pubky_app_specs::{
    traits::TimestampId, PubkyAppFile, PubkyAppFollow, PubkyAppPost, PubkyAppUser,
};
use pubky_common::crypto::Keypair;
use pubky_homeserver::Homeserver;
use pubky_nexus::events::retry::manager::{RetryManager, CHANNEL_BUFFER};
use pubky_nexus::events::Event;
use pubky_nexus::types::{DynError, PubkyId};
use pubky_nexus::{setup, Config, EventProcessor, PubkyConnector};
use serde_json::to_vec;
use tokio::sync::mpsc;

/// Struct to hold the setup environment for tests
pub struct WatcherTest {
    pub homeserver: Homeserver,
    pub event_processor: EventProcessor,
    pub config: Config,
    pub ensure_event_processing: bool,
}

impl WatcherTest {
    /// Sets up the test environment for the watcher.
    ///
    /// This function performs the following steps:
    /// 1. Reads configuration from environment variables.
    /// 2. Initializes database connectors for Neo4j and Redis.
    /// 3. Sets up the global DHT test network for the watcher.
    /// 4. Creates and starts a test homeserver instance.
    /// 5. Initializes a retry manager and ensures robustness by managing retries asynchronously.
    /// 6. Initializes the PubkyConnector with the configuration and global test DHT nodes.
    /// 7. Creates and configures the event processor with the homeserver URL.
    ///
    /// # Returns
    /// Returns an instance of `Self` containing the configuration, homeserver,
    /// event processor, and other test setup details.
    pub async fn setup() -> Result<Self> {
        let config = Config::from_env();
        setup(&config).await;

        TestnetDHTNetwork::initialise(10)?;
        let testnet = TestnetDHTNetwork::get_testnet_dht_nodes()?;

        let homeserver = Homeserver::start_test(&testnet).await?;
        let homeserver_url = format!("http://localhost:{}", homeserver.port());

        let retry_manager = RetryManager::initialise(mpsc::channel(CHANNEL_BUFFER));

        let sender_clone = retry_manager.sender.clone();

        tokio::spawn(async move {
            let _ = retry_manager.exec().await;
        });

        match PubkyConnector::initialise(&config, Some(&testnet)) {
            Ok(_) => debug!("WatcherTest: PubkyConnector initialised"),
            Err(e) => debug!("WatcherTest: {}", e),
        }

        let homeserver_pubky = homeserver.public_key().to_uri_string();
        // Slice after the 3rd character
        let pubky_part = &homeserver_pubky["pk:".len()..];
        // Not save,
        let homeserver_pubky =
            PubkyId::try_from(&pubky_part).expect("PubkyId: Cannot get the homeserver public key");

        let event_processor =
            EventProcessor::test(homeserver_url, homeserver_pubky, sender_clone).await;

        Ok(Self {
            config,
            homeserver,
            event_processor,
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
            self.event_processor.run().await.map_err(|e| anyhow!(e))?;
            // tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        Ok(())
    }

    pub fn _get_homeserver_pubky(&self) -> PubkyId {
        let homeserver_pubky = self.homeserver.public_key().to_uri_string();
        // Slice after the 3rd character
        let pubky_part = &homeserver_pubky["pk:".len()..];
        // Not save,
        PubkyId::try_from(&pubky_part).expect("PubkyId: Cannot get the homeserver public key")
    }

    /// Sends a PUT request to the homeserver with the provided blob of data.
    ///
    /// This function performs the following steps:
    /// 1. Retrieves the Pubky client from the PubkyConnector.
    /// 2. Sends the blob data to the specified homeserver URI using a PUT request.
    /// 3. Ensures that all event processing is complete after the PUT operation.
    ///
    /// # Parameters
    /// - `homeserver_uri`: The URI of the homeserver to write the data to.
    /// - `blob`: A vector of bytes representing the data to be sent.
    pub async fn put(&mut self, homeserver_uri: &str, blob: Vec<u8>) -> Result<()> {
        let pubky_client = PubkyConnector::get_pubky_client()?;
        pubky_client.put(homeserver_uri, &blob).await?;
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
        let pubky_client = PubkyConnector::get_pubky_client()?;
        pubky_client.delete(homeserver_uri).await?;
        self.ensure_event_processing_complete().await?;
        Ok(())
    }

    /// Registers a user in the homeserver with the keypair.
    /// # Arguments
    /// * `keypair` - A reference to the `Keypair` used for signing up the user.
    pub async fn register_user(&self, keypair: &Keypair) -> Result<()> {
        let pubky_client = PubkyConnector::get_pubky_client()?;

        pubky_client
            .signup(&keypair, &self.homeserver.public_key())
            .await?;
        Ok(())
    }

    pub async fn create_user(&mut self, keypair: &Keypair, user: &PubkyAppUser) -> Result<String> {
        let user_id = keypair.public_key().to_z32();
        let pubky_client = PubkyConnector::get_pubky_client()?;
        // Register the key in the homeserver
        pubky_client
            .signup(keypair, &self.homeserver.public_key())
            .await?;

        let profile_json = to_vec(user)?;
        let url = format!("pubky://{}/pub/pubky.app/profile.json", user_id);

        // Write the user profile in the pubky.app repository
        pubky_client.put(url.as_str(), &profile_json).await?;

        // Index to Nexus from Homeserver using the events processor
        self.ensure_event_processing_complete().await?;
        Ok(user_id)
    }

    pub async fn create_post(&mut self, user_id: &str, post: &PubkyAppPost) -> Result<String> {
        let post_id = post.create_id();
        let post_json = to_vec(post)?;
        let url = format!("pubky://{}/pub/pubky.app/posts/{}", user_id, post_id);
        // Write the post in the pubky.app repository
        PubkyConnector::get_pubky_client()?
            .put(url.as_str(), &post_json)
            .await?;

        // Index to Nexus from Homeserver using the events processor
        self.ensure_event_processing_complete().await?;
        Ok(post_id)
    }

    pub async fn cleanup_user(&mut self, user_id: &str) -> Result<()> {
        let url = format!("pubky://{}/pub/pubky.app/profile.json", user_id);
        PubkyConnector::get_pubky_client()?
            .delete(url.as_str())
            .await?;
        self.ensure_event_processing_complete().await?;
        Ok(())
    }

    pub async fn cleanup_post(&mut self, user_id: &str, post_id: &str) -> Result<()> {
        let url = format!("pubky://{}/pub/pubky.app/posts/{}", user_id, post_id);
        PubkyConnector::get_pubky_client()?
            .delete(url.as_str())
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
        let file_json = to_vec(file)?;
        let url = format!("pubky://{}/pub/pubky.app/files/{}", user_id, file_id);
        PubkyConnector::get_pubky_client()?
            .put(url.as_str(), &file_json)
            .await?;

        self.ensure_event_processing_complete().await?;
        Ok((file_id, url))
    }

    pub async fn cleanup_file(&mut self, user_id: &str, file_id: &str) -> Result<()> {
        let url = format!("pubky://{}/pub/pubky.app/files/{}", user_id, file_id);
        PubkyConnector::get_pubky_client()?
            .delete(url.as_str())
            .await?;
        self.ensure_event_processing_complete().await?;
        Ok(())
    }

    pub async fn create_follow(&mut self, follower_id: &str, followee_id: &str) -> Result<String> {
        let follow_relationship = PubkyAppFollow {
            created_at: Utc::now().timestamp_millis(),
        };
        let blob = serde_json::to_vec(&follow_relationship)?;
        let follow_url = format!(
            "pubky://{}/pub/pubky.app/follows/{}",
            follower_id, followee_id
        );
        PubkyConnector::get_pubky_client()?
            .put(follow_url.as_str(), &blob)
            .await?;
        // Process the event
        self.ensure_event_processing_complete().await?;
        Ok(follow_url)
    }

    pub async fn create_mute(&mut self, muter_id: &str, mutee_id: &str) -> Result<String> {
        let mute_relationship = PubkyAppFollow {
            created_at: Utc::now().timestamp_millis(),
        };
        let blob = serde_json::to_vec(&mute_relationship)?;
        let mute_url = format!("pubky://{}/pub/pubky.app/mutes/{}", muter_id, mutee_id);
        PubkyConnector::get_pubky_client()?
            .put(mute_url.as_str(), &blob)
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
pub async fn retrieve_and_handle_event_line(event_line: &str) -> Result<(), DynError> {
    let event = match Event::parse_event(event_line) {
        Ok(event) => event,
        Err(_) => None,
    };

    if let Some(event) = event {
        event.clone().handle().await?
    }

    Ok(())
}
