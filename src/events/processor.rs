use crate::Config;
use log::{debug, info};
use pkarr::mainline::Testnet;
use pubky::PubkyClient;
use reqwest::Client;

use super::Event;

pub struct EventProcessor {
    pubky_client: PubkyClient,
    http_client: Client,
    homeserver_url: String, // Ideally should only need the homeserver_pk
    cursor: String,
    limit: u32,
}

impl EventProcessor {
    pub async fn from_config(config: &Config) -> Self {
        let pubky_client = match config.testnet {
            true => {
                let testnet = Testnet {
                    bootstrap: vec![config.bootstrap.clone()],
                    nodes: vec![],
                };
                PubkyClient::test(&testnet)
            }
            false => PubkyClient::new(),
        };

        Self {
            pubky_client,
            http_client: Client::new(),
            homeserver_url: config.homeserver_url.clone(),
            cursor: "0".to_string(),
            limit: config.events_limit,
        }
    }

    pub async fn test(testnet: &Testnet, homeserver_url: String) -> Self {
        Self {
            pubky_client: PubkyClient::test(testnet),
            http_client: Client::new(),
            homeserver_url,
            cursor: "0".to_string(),
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
                self.homeserver_url, self.cursor, self.limit
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
                    self.cursor = cursor.to_string();
                    info!("Cursor for the next request: {}", cursor);
                }
            } else if let Some(event) = Event::from_str(line, self.pubky_client.clone()) {
                event.handle().await?;
            }
        }

        Ok(())
    }
}
