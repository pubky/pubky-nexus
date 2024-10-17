use std::time::Duration;

use super::Event;
use crate::types::DynError;
use crate::types::PubkyId;
use crate::{models::homeserver::Homeserver, Config};
use log::{debug, error, info};
use pkarr::mainline::dht::Testnet;
use pubky::PubkyClient;
use reqwest::Client;

pub struct EventProcessor {
    pubky_client: PubkyClient,
    http_client: Client,
    homeserver: Homeserver,
    limit: u32,
    max_retries: u64,
}

impl EventProcessor {
    pub async fn from_config(config: &Config) -> Result<Self, DynError> {
        let pubky_client = Self::init_pubky_client(config);
        let homeserver = Homeserver::from_config(config).await?;
        let limit = config.events_limit;
        let max_retries = config.max_retries;

        info!(
            "Initialized Event Processor for homeserver: {:?}",
            homeserver
        );

        Ok(Self {
            pubky_client,
            http_client: Client::new(),
            homeserver,
            limit,
            max_retries,
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
        let id = PubkyId("test".to_string());
        let homeserver = Homeserver::new(id, homeserver_url).await.unwrap();
        Self {
            pubky_client: PubkyClient::builder().testnet(testnet).build(),
            http_client: Client::new(),
            homeserver,
            limit: 1000,
            max_retries: 3,
        }
    }

    pub async fn run(&mut self) -> Result<(), DynError> {
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

    async fn process_event_lines(&mut self, lines: Vec<String>) -> Result<(), DynError> {
        for line in &lines {
            if line.starts_with("cursor:") {
                if let Some(cursor) = line.strip_prefix("cursor: ") {
                    self.homeserver.cursor = cursor.to_string();
                    self.homeserver.put_to_index().await?;
                    info!("Cursor for the next request: {}", cursor);
                }
            } else {
                let event = match Event::from_str(line, self.pubky_client.clone()) {
                    Ok(event) => event,
                    Err(e) => {
                        error!("Error while creating event line from line: {}", e);
                        None
                    }
                };
                if let Some(event) = event {
                    debug!("Processing event: {:?}", event);
                    self.handle_event_with_retry(event).await?;
                }
            }
        }

        Ok(())
    }

    // Generic retry on event handler
    async fn handle_event_with_retry(&self, event: Event) -> Result<(), DynError> {
        let mut attempts: u32 = 0;
        let delay: u32 = 200;
        let backoff: u32 = 2;
        loop {
            match event.clone().handle().await {
                Ok(_) => break Ok(()),
                Err(e) => {
                    attempts += 1;
                    if u64::from(attempts) >= self.max_retries {
                        error!(
                            "Error while handling event after {} attempts: {}",
                            attempts, e
                        );
                        break Ok(());
                    } else {
                        error!(
                            "Error while handling event: {}. Retrying attempt {}/{}",
                            e, attempts, self.max_retries
                        );
                        // Optionally, add a delay between retries
                        let attempt_delay =
                            u64::from(delay * backoff.pow(u32::from(attempts * 2) - 1));
                        tokio::time::sleep(Duration::from_millis(attempt_delay)).await;
                    }
                }
            }
        }
    }
}
