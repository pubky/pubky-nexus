use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use pkarr::mainline::Testnet;
use pubky_app_specs::{
    traits::TimestampId, PubkyAppFile, PubkyAppFollow, PubkyAppPost, PubkyAppUser,
};
use pubky_common::crypto::Keypair;
use pubky_homeserver::Homeserver;
use pubky_nexus::{setup, Config, EventProcessor, PubkyConnector};
use serde_json::to_vec;
use tokio::sync::OnceCell;

/// Struct to hold the setup environment for tests
pub struct WatcherTest {
    pub homeserver: Homeserver,
    pub event_processor: EventProcessor,
    pub config: Config,
    pub ensure_event_processing: bool,
}

impl WatcherTest {
    pub async fn setup() -> Result<Self> {
        let config = Config::from_env();
        setup(&config).await;

        // Initialise watcher DHT network
        TestnetDHTNetwork::initialise().unwrap();
        let testnet = TestnetDHTNetwork::get_testnet_dht_nodes().unwrap();
        let homeserver = Homeserver::start_test(&testnet).await?;

        // Initialise connector
        match PubkyConnector::initialise(&config, Some(&testnet)) {
            Ok(_) => println!("PubkyConnector initialised"),
            Err(_) => println!("PubkyConnector already initialised")
        }
        let homeserver_url = format!("http://localhost:{}", homeserver.port());

        let event_processor = EventProcessor::test(homeserver_url).await;

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
            self.event_processor
                .run()
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            // tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        Ok(())
    }

    pub async fn create_user(&mut self, keypair: &Keypair, user: &PubkyAppUser) -> Result<String> {
        let user_id = keypair.public_key().to_z32();
        let pubky_client = PubkyConnector::get_pubky_client()?;
        pubky_client
            .signup(keypair, &self.homeserver.public_key())
            .await?;

        let profile_json = to_vec(user)?;
        let url = format!("pubky://{}/pub/pubky.app/profile.json", user_id);
        let pubky_client = PubkyConnector::get_pubky_client()?;
        pubky_client.put(url.as_str(), &profile_json).await?;

        // Index to Nexus from Homeserver using the events processor
        self.ensure_event_processing_complete().await?;
        Ok(user_id)
    }

    pub async fn create_post(&mut self, user_id: &str, post: &PubkyAppPost) -> Result<String> {
        let post_id = post.create_id();
        let post_json = to_vec(post)?;
        let url = format!("pubky://{}/pub/pubky.app/posts/{}", user_id, post_id);
        let pubky_client = PubkyConnector::get_pubky_client()?;
        pubky_client.put(url.as_str(), &post_json).await?;

        // Index to Nexus from Homeserver using the events processor
        self.ensure_event_processing_complete().await?;
        Ok(post_id)
    }

    pub async fn create_tag(&mut self, tag_url: &str, tag_blob: Vec<u8>) -> Result<()> {
        let pubky_client = PubkyConnector::get_pubky_client()?;
        pubky_client.put(tag_url, &tag_blob).await?;
        self.ensure_event_processing_complete().await?;
        Ok(())
    }

    pub async fn delete_tag(&mut self, tag_url: &str) -> Result<()> {
        let pubky_client = PubkyConnector::get_pubky_client()?;
        pubky_client.delete(tag_url).await?;
        self.ensure_event_processing_complete().await?;
        Ok(())
    }

    pub async fn create_bookmark(
        &mut self,
        bookmark_url: &str,
        bookmark_blob: Vec<u8>,
    ) -> Result<()> {
        let pubky_client = PubkyConnector::get_pubky_client()?;
        pubky_client.put(bookmark_url, &bookmark_blob).await?;
        self.ensure_event_processing_complete().await?;
        Ok(())
    }

    pub async fn delete_bookmark(&mut self, bookmark_url: &str) -> Result<()> {
        let pubky_client = PubkyConnector::get_pubky_client()?;
        pubky_client.delete(bookmark_url).await?;
        self.ensure_event_processing_complete().await
    }

    pub async fn cleanup_user(&mut self, user_id: &str) -> Result<()> {
        let url = format!("pubky://{}/pub/pubky.app/profile.json", user_id);
        let pubky_client = PubkyConnector::get_pubky_client()?;
        pubky_client.delete(url.as_str()).await?;
        self.ensure_event_processing_complete().await?;
        Ok(())
    }

    pub async fn cleanup_post(&mut self, user_id: &str, post_id: &str) -> Result<()> {
        let url = format!("pubky://{}/pub/pubky.app/posts/{}", user_id, post_id);
        let pubky_client = PubkyConnector::get_pubky_client()?;
        pubky_client.delete(url.as_str()).await?;
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
        let pubky_client = PubkyConnector::get_pubky_client()?;
        pubky_client.put(url.as_str(), &file_json).await?;

        self.ensure_event_processing_complete().await?;
        Ok((file_id, url))
    }

    pub async fn cleanup_file(&mut self, user_id: &str, file_id: &str) -> Result<()> {
        let url = format!("pubky://{}/pub/pubky.app/files/{}", user_id, file_id);
        let pubky_client = PubkyConnector::get_pubky_client()?;
        pubky_client.delete(url.as_str()).await?;
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
        let pubky_client = PubkyConnector::get_pubky_client()?;
        pubky_client.put(follow_url.as_str(), &blob).await?;
        // Process the event
        self.ensure_event_processing_complete().await?;
        Ok(follow_url)
    }

    pub async fn delete_follow(&mut self, follow_url: &str) -> Result<()> {
        let pubky_client = PubkyConnector::get_pubky_client()?;
        pubky_client.delete(follow_url).await?;
        // Process the event
        self.ensure_event_processing_complete().await?;
        Ok(())
    }

    pub async fn create_mute(&mut self, muter_id: &str, mutee_id: &str) -> Result<String> {
        let mute_relationship = PubkyAppFollow {
            created_at: Utc::now().timestamp_millis(),
        };
        let blob = serde_json::to_vec(&mute_relationship)?;
        let mute_url = format!("pubky://{}/pub/pubky.app/mutes/{}", muter_id, mutee_id);
        let pubky_client = PubkyConnector::get_pubky_client()?;
        pubky_client.put(mute_url.as_str(), &blob).await?;
        // Process the event
        self.ensure_event_processing_complete().await?;
        Ok(mute_url)
    }

    pub async fn delete_mute(&mut self, mute_url: &str) -> Result<()> {
        let pubky_client = PubkyConnector::get_pubky_client()?;
        pubky_client.delete(mute_url).await?;
        // Process the event
        self.ensure_event_processing_complete().await?;
        Ok(())
    }
}


// TODO: TIdy up that one
static DHT_TESTNET_NETWORK_SINGLETON: OnceCell<TestnetDHTNetwork> = OnceCell::const_new();
struct TestnetDHTNetwork {
    nodes: Arc<Testnet>
}

impl TestnetDHTNetwork {
    pub fn initialise() -> Result<(), &'static str>{
        if DHT_TESTNET_NETWORK_SINGLETON.get().is_some() {
            return Ok(());
        }
        let testnet = Self {
            nodes: Arc::new(Testnet::new(10)),
        };
        DHT_TESTNET_NETWORK_SINGLETON
            .set(testnet)
            .map_err(|_| "Already initiailsed")?;
        Ok(())
    }

    pub fn get_testnet_dht_nodes() -> Result<Arc<Testnet>, &'static str> {
        if let Some(resolver) = DHT_TESTNET_NETWORK_SINGLETON.get() {
            Ok(resolver.nodes.clone())
        } else {
            Err("PubkyConnectorError::NotInitialized")
        }
    }
}