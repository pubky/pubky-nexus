use anyhow::Result;
use once_cell::sync::OnceCell;
use pkarr::{mainline::Testnet, Keypair};
use pubky::PubkyClient;
use pubky_homeserver::Homeserver;
use pubky_nexus::{
    models::pubky_app::{traits::GenerateId, PubkyAppPost, PubkyAppUser},
    setup, Config, EventProcessor,
};
use serde_json::to_vec;
use std::sync::Arc;
use std::sync::Once;
use std::time::Duration;

pub struct WatcherTest {
    pub homeserver: Arc<Homeserver>,
    pub client: Arc<PubkyClient>,
    pub event_processor: EventProcessor,
}

static INIT: Once = Once::new();
static HOMESERVER: OnceCell<Arc<Homeserver>> = OnceCell::new();
static PUBKY_CLIENT: OnceCell<Arc<PubkyClient>> = OnceCell::new();

impl WatcherTest {
    pub async fn setup_test() -> Result<Self> {
        INIT.call_once(|| {
            tokio::spawn(async {
                let config = Config::from_env();
                setup(&config).await;

                let testnet = Testnet::new(10);
                let homeserver = Homeserver::start_test(&testnet).await.unwrap();
                let client = PubkyClient::builder()
                    .testnet(&testnet)
                    .dht_request_timeout(Duration::from_millis(2000))
                    .build();

                HOMESERVER.set(Arc::new(homeserver)).unwrap();
                PUBKY_CLIENT.set(Arc::new(client)).unwrap();
            });
        });

        let homeserver = HOMESERVER
            .get()
            .expect("Homeserver is not initialized")
            .clone();
        let client = PUBKY_CLIENT
            .get()
            .expect("PubkyClient is not initialized")
            .clone();

        let homeserver_url = format!("http://localhost:{}", homeserver.port());
        let event_processor = EventProcessor::test(&Testnet::new(10), homeserver_url).await;

        Ok(Self {
            homeserver,
            client,
            event_processor,
        })
    }

    // The rest of the methods remain the same...
    pub async fn ensure_event_processing_complete(&mut self) -> Result<()> {
        self.event_processor
            .run()
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await; // Ensure completion
        Ok(())
    }

    pub async fn create_user(&mut self, keypair: &Keypair, user: &PubkyAppUser) -> Result<String> {
        let user_id = keypair.public_key().to_z32();
        self.client
            .signup(keypair, &self.homeserver.public_key())
            .await?;

        let profile_json = to_vec(user)?;
        let url = format!("pubky://{}/pub/pubky-app/profile.json", user_id);
        self.client.put(url.as_str(), &profile_json).await?;

        // Index to Nexus from Homeserver using the events processor
        self.ensure_event_processing_complete().await?;
        Ok(user_id)
    }

    pub async fn create_post(&mut self, user_id: &str, post: &PubkyAppPost) -> Result<String> {
        let post_id = post.create_id();
        let post_json = to_vec(post)?;
        let url = format!("pubky://{}/pub/pubky-app/posts/{}", user_id, post_id);
        self.client.put(url.as_str(), &post_json).await?;

        // Index to Nexus from Homeserver using the events processor
        self.ensure_event_processing_complete().await?;
        Ok(post_id)
    }

    pub async fn cleanup_user(&mut self, user_id: &str) -> Result<()> {
        let url = format!("pubky://{}/pub/pubky-app/profile.json", user_id);
        self.client.delete(url.as_str()).await?;
        self.ensure_event_processing_complete().await?;
        Ok(())
    }

    pub async fn cleanup_post(&mut self, user_id: &str, post_id: &str) -> Result<()> {
        let url = format!("pubky://{}/pub/pubky-app/posts/{}", user_id, post_id);
        self.client.delete(url.as_str()).await?;
        self.ensure_event_processing_complete().await?;
        Ok(())
    }
}
