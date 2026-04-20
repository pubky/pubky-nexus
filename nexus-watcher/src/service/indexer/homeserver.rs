use super::TEventProcessor;
use crate::events::retry::RetryScheduler;
use crate::events::{EventHandler, TModeration};
use nexus_common::db::PubkyConnector;
use nexus_common::models::event::{Event, EventProcessorError, ParseResult};
use nexus_common::models::homeserver::Homeserver;
use pubky::Method;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch::Receiver;
use tracing::{debug, error, info, warn};

/// Event processor for the default homeserver
pub struct HsEventProcessor {
    /// The default HS endpoint this processor fetches events from
    pub homeserver: Homeserver,

    /// See [WatcherConfig::events_limit]
    pub limit: u32,
    pub files_path: PathBuf,
    pub moderation: Arc<dyn TModeration>,
    pub event_handler: Arc<dyn EventHandler>,
    pub shutdown_rx: Receiver<bool>,
    /// Scheduler used to enqueue failed events onto the retry queue
    pub retry_scheduler: Arc<RetryScheduler>,
}

#[async_trait::async_trait]
impl TEventProcessor for HsEventProcessor {
    fn files_path(&self) -> &PathBuf {
        &self.files_path
    }

    fn moderation(&self) -> &Arc<dyn TModeration> {
        &self.moderation
    }

    fn event_handler(&self) -> &Arc<dyn EventHandler> {
        &self.event_handler
    }

    fn instance_name(&self) -> String {
        format!("HsEventProcessor with HS ID: {}", self.homeserver.id)
    }

    fn retry_scheduler(&self) -> &Arc<RetryScheduler> {
        &self.retry_scheduler
    }

    async fn run_internal(self: Arc<Self>) -> Result<(), EventProcessorError> {
        let maybe_event_lines = self
            .poll_events()
            .await
            .inspect_err(|e| error!("Error polling events: {e:?}"))?;

        match maybe_event_lines {
            None => debug!("No new events"),
            Some(event_lines) => {
                info!("Processing {} event lines", event_lines.len());
                self.process_event_lines(event_lines).await?;
            }
        }

        Ok(())
    }
}

impl HsEventProcessor {
    /// Polls new events from the homeserver.
    ///
    /// It sends a GET request to the homeserver's events endpoint
    /// using the current cursor and a specified limit. It retrieves new event
    /// URIs in a newline-separated format, processes it into a vector of strings,
    /// and returns the result.
    #[tracing::instrument(name = "events.poll", skip_all, fields(homeserver = %self.homeserver.id))]
    async fn poll_events(&self) -> Result<Option<Vec<String>>, EventProcessorError> {
        debug!("Polling new events from homeserver");

        let response_text = {
            let pubky = PubkyConnector::get()?;
            let url = format!(
                "https://{}/events/?cursor={}&limit={}",
                self.homeserver.id, self.homeserver.cursor, self.limit
            );

            let response = pubky
                .client()
                .request(Method::GET, &url)
                .send()
                .await
                .map_err(|e| EventProcessorError::client_error(e.to_string()))?;

            response
                .text()
                .await
                .map_err(|e| EventProcessorError::client_error(e.to_string()))?
        };

        let lines: Vec<String> = response_text.trim().lines().map(String::from).collect();
        debug!("Homeserver response lines {:?}", lines);

        if lines.is_empty() || (lines.len() == 1 && lines[0].is_empty()) {
            return Ok(None);
        }

        Ok(Some(lines))
    }

    /// Processes a batch of event lines retrieved from the homeserver.
    ///
    /// This function implements the retry logic:
    /// - On infrastructure error: stops the batch, cursor is not saved, next tick replays from same position
    /// - On MissingDependency: stores event in retry queue, continues processing
    /// - On 404 (blob not found): skips indexing, continues processing
    /// - On InvalidEventLine/SkipIndexing: logs and continues
    ///
    /// # Parameters
    /// - `lines`: A vector of strings representing event lines retrieved from the homeserver.
    #[tracing::instrument(name = "event_batch.process", skip_all, fields(batch.size = lines.len()))]
    pub async fn process_event_lines(&self, lines: Vec<String>) -> Result<(), EventProcessorError> {
        for line in &lines {
            let id = self.homeserver.id.clone();

            if *self.shutdown_rx.borrow() {
                debug!(hs_id = %id, "Shutdown detected; exiting event processing loop");
                return Ok(());
            }

            if let Some(cursor) = line.strip_prefix("cursor: ") {
                // Batch complete - save cursor
                info!("Received cursor for the next request: {cursor}");
                match Homeserver::try_from_cursor(id, cursor) {
                    Ok(hs) => {
                        hs.put_to_index().await?;
                    }
                    Err(e) => warn!("{e}"),
                }
                continue;
            }

            // Process the event line
            match Event::parse_event(line, self.files_path.clone()) {
                Ok(ParseResult::Parsed(event)) => {
                    debug!("Processing event: {:?}", event);

                    // Use the trait's handle_event method which delegates to handle_error
                    match self.handle_event(&event).await {
                        Ok(()) => {
                            // Event processed successfully
                        }
                        Err(e) => {
                            // Infrastructure error - stop the batch
                            error!("Infrastructure error processing event: {e}");
                            return Err(e);
                        }
                    }
                }
                Ok(ParseResult::Skipped) => {
                    // Skipped event - continue
                }
                Ok(ParseResult::UnrecognizedUri { reason, .. }) => {
                    // Should not normally occur — UnknownResource parsing happens in HomeserverParsedUri
                    warn!("Unrecognized event URI: {reason}");
                }
                Err(e) => warn!("Failed to parse event line: {e}"),
            }
        }

        Ok(())
    }
}
