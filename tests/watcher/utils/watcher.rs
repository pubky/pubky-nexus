use super::dht::TestnetDHTNetwork;
use anyhow::{anyhow, Result};
use chrono::Utc;
use log::debug;
use pubky_app_specs::{
    traits::TimestampId, PubkyAppFile, PubkyAppFollow, PubkyAppPost, PubkyAppUser,
};
use pubky_common::crypto::Keypair;
use pubky_homeserver::Homeserver;
use pubky_nexus::{events::Event, setup, types::DynError, Config, EventProcessor, PubkyConnector};

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
    /// 5. Initializes the PubkyConnector with the configuration and global test DHT nodes.
    /// 6. Creates and configures the event processor with the homeserver URL.
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
        let homeserver_id = homeserver.public_key().to_string();

        match PubkyConnector::initialise(&config, Some(&testnet)) {
            Ok(_) => debug!("WatcherTest: PubkyConnector initialised"),
            Err(e) => debug!("WatcherTest: {}", e),
        }

        let event_processor = EventProcessor::test(homeserver_id).await;

        Ok(Self {
            config,
            homeserver,
            event_processor,
            ensure_event_processing: true,
        })
    }

    pub async fn remove_event_processing(mut self) -> Self {
        self.ensure_event_processing = false;
        self
    }

    pub async fn ensure_event_processing_complete(&mut self) -> Result<()> {
        if self.ensure_event_processing {
            self.event_processor.run().await.map_err(|e| anyhow!(e))?;
            // tokio::time::sleep(std::time::Duration::from_millis(50)).await;
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
        let pubky_client = PubkyConnector::get_pubky_client()?;
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
        let pubky_client = PubkyConnector::get_pubky_client()?;
        pubky_client.delete(homeserver_uri).send().await?;
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
        let url = format!("pubky://{}/pub/pubky.app/profile.json", user_id);

        // Write the user profile in the pubky.app repository
        pubky_client.put(url.as_str()).json(&user).send().await?;

        // Index to Nexus from Homeserver using the events processor
        self.ensure_event_processing_complete().await?;
        Ok(user_id)
    }

    pub async fn create_post(&mut self, user_id: &str, post: &PubkyAppPost) -> Result<String> {
        let post_id = post.create_id();
        let url = format!("pubky://{}/pub/pubky.app/posts/{}", user_id, post_id);
        // Write the post in the pubky.app repository
        PubkyConnector::get_pubky_client()?
            .put(url.as_str())
            .json(&post)
            .send()
            .await?;

        // Index to Nexus from Homeserver using the events processor
        self.ensure_event_processing_complete().await?;
        Ok(post_id)
    }

    pub async fn cleanup_user(&mut self, user_id: &str) -> Result<()> {
        let url = format!("pubky://{}/pub/pubky.app/profile.json", user_id);
        PubkyConnector::get_pubky_client()?
            .delete(url.as_str())
            .send()
            .await?;
        self.ensure_event_processing_complete().await?;
        Ok(())
    }

    pub async fn cleanup_post(&mut self, user_id: &str, post_id: &str) -> Result<()> {
        let url = format!("pubky://{}/pub/pubky.app/posts/{}", user_id, post_id);
        PubkyConnector::get_pubky_client()?
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
        let url = format!("pubky://{}/pub/pubky.app/files/{}", user_id, file_id);
        PubkyConnector::get_pubky_client()?
            .put(url.as_str())
            .json(&file)
            .send()
            .await?;

        self.ensure_event_processing_complete().await?;
        Ok((file_id, url))
    }

    pub async fn cleanup_file(&mut self, user_id: &str, file_id: &str) -> Result<()> {
        let url = format!("pubky://{}/pub/pubky.app/files/{}", user_id, file_id);
        PubkyConnector::get_pubky_client()?
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
        let follow_url = format!(
            "pubky://{}/pub/pubky.app/follows/{}",
            follower_id, followee_id
        );
        PubkyConnector::get_pubky_client()?
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
        let mute_url = format!("pubky://{}/pub/pubky.app/mutes/{}", muter_id, mutee_id);
        PubkyConnector::get_pubky_client()?
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
