use std::time::Duration;

use super::Event;
use crate::types::DynError;
use crate::types::PubkyId;
use crate::{models::homeserver::Homeserver, Config};
use log::{debug, error, info};
use reqwest::Client;

pub struct EventProcessor {
    http_client: Client,
    homeserver: Homeserver,
    limit: u32,
    max_retries: u64,
}

impl EventProcessor {
    pub async fn from_config(config: &Config) -> Result<Self, DynError> {
        let homeserver = Homeserver::from_config(config).await?;
        let limit = config.events_limit;
        let max_retries = config.max_retries;

        info!(
            "Initialized Event Processor for homeserver: {:?}",
            homeserver
        );

        Ok(Self {
            http_client: Client::new(),
            homeserver,
            limit,
            max_retries,
        })
    }

    /// Creates a new `EventProcessor` instance for testing purposes.
    ///
    /// Initializes an `EventProcessor` with a mock homeserver and a default configuration,
    /// making it suitable for use in integration tests and benchmarking scenarios.
    ///
    /// # Parameters
    /// - `homeserver_url`: The URL of the homeserver to be used in the test environment.
    pub async fn test(homeserver_url: String) -> Self {
        let id = PubkyId("test".to_string());
        let homeserver = Homeserver::new(id, homeserver_url).await.unwrap();
        Self {
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

    pub async fn process_event_lines(&mut self, lines: Vec<String>) -> Result<(), DynError> {
        for line in &lines {
            if line.starts_with("cursor:") {
                if let Some(cursor) = line.strip_prefix("cursor: ") {
                    self.homeserver.cursor = cursor.to_string();
                    self.homeserver.put_to_index().await?;
                    info!("Cursor for the next request: {}", cursor);
                }
            } else {
                let event = match Event::parse_event(line) {
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
        let mut attempts = 0;
        loop {
            match event.clone().handle().await {
                Ok(_) => break Ok(()),
                Err(e) => {
                    // TODO: Failing to index the profile.json, the error message might be different
                    // WIP: It will be fixed in the comming PRs the error messages
                    if e.to_string() != "WATCHER: Missing some dependency to index the model" {
                        attempts += 1;
                        if attempts >= self.max_retries {
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
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                    } else {
                        error!("PROCESSOR: Sending the event to RetryManager... Missing node(s) and/or relationship(s) to execute PUT or DEL operation(s)");
                        return Ok(());
                    }
                }
            }
        }
    }
}
