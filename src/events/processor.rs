use super::Event;
use crate::{
    models::{homeserver::Homeserver, user::PubkyId},
    Config,
};
use log::{debug, error, info};
use pkarr::mainline::dht::Testnet;
use pubky::PubkyClient;
use reqwest::Client;

const MAX_RETRIES: usize = 3;

pub struct EventProcessor {
    pubky_client: PubkyClient,
    http_client: Client,
    homeserver: Homeserver,
    limit: u32,
}

impl EventProcessor {
    pub async fn from_config(
        config: &Config,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let pubky_client = Self::init_pubky_client(config);
        let homeserver = Homeserver::from_config(config).await?;
        let limit = config.events_limit;

        info!(
            "Initialized Event Processor for homeserver: {:?}",
            homeserver
        );

        Ok(Self {
            pubky_client,
            http_client: Client::new(),
            homeserver,
            limit,
        })
    }

    fn init_pubky_client(config: &Config) -> PubkyClient {
        if config.testnet {
            let testnet = Testnet {
                bootstrap: vec![config.bootstrap.clone()],
                nodes: vec![],
            };
            PubkyClient::test(&testnet)
        } else {
            PubkyClient::default()
        }
    }

    pub async fn test(testnet: &Testnet, homeserver_url: String) -> Self {
        let mut id = PubkyId::default();
        id.0 = "test".to_string();
        let homeserver = Homeserver::new(id, homeserver_url).await.unwrap();
        Self {
            pubky_client: PubkyClient::builder().testnet(testnet).build(),
            http_client: Client::new(),
            homeserver,
            limit: 100,
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let lines = { self.poll_events().await.unwrap_or_default() };
        if let Some(lines) = lines {
            self.process_event_lines(lines).await?;
        };
        Ok(())
    }

    async fn poll_events(&mut self) -> Result<Option<Vec<String>>, Box<dyn std::error::Error>> {
        debug!("Polling new events from homeserver");
        let res = self
            .http_client
            .get(format!(
                "{}/events/?cursor={}&limit={}",
                self.homeserver.url, self.homeserver.cursor, self.limit
            ))
            .send()
            .await?
            .text()
            .await?;

        let lines: Vec<String> = res.trim().split('\n').map(|s| s.to_string()).collect();
        debug!("Homeserver response lines {:?}", lines);

        if lines.len() == 1 && lines[0].is_empty() {
            info!("No new events");
            Ok(None)
        } else {
            Ok(Some(lines))
        }
    }

    async fn process_event_lines(
        &mut self,
        lines: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for line in &lines {
            if line.starts_with("cursor:") {
                if let Some(cursor) = line.strip_prefix("cursor: ") {
                    self.homeserver.cursor = cursor.to_string();
                    self.homeserver.put_to_index().await?;
                    info!("Cursor for the next request: {}", cursor);
                }
            } else if let Some(event) = Event::from_str(line, self.pubky_client.clone())? {
                debug!("Processing event: {:?}", event);
                self.handle_event_with_retry(event).await?;
            }
        }

        Ok(())
    }

    // Generic retry on event handler
    async fn handle_event_with_retry(
        &self,
        event: Event,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut attempts = 0;
        loop {
            match event.clone().handle().await {
                Ok(_) => break Ok(()),
                Err(e) => {
                    attempts += 1;
                    if attempts >= MAX_RETRIES {
                        error!(
                            "Error while handling event after {} attempts: {}",
                            attempts, e
                        );
                        break Ok(());
                    } else {
                        error!(
                            "Error while handling event: {}. Retrying attempt {}/{}",
                            e, attempts, MAX_RETRIES
                        );
                        // Optionally, add a delay between retries
                        // tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        }
    }
}
