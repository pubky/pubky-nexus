use super::TEventProcessor;
use crate::events::retry::RetryScheduler;
use crate::events::EventHandler;
use crate::service::user_hs_resolver::PkdnsHomeserverResolver;
use nexus_common::db::{fetch_key_from_graph, queries, PubkyConnector};
use nexus_common::models::event::{Event, EventProcessorError};
use nexus_common::models::homeserver::Homeserver;
use pubky::Method;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch::Receiver;
use tracing::{debug, error, info, warn};

/// Event processor for the default homeserver
pub struct HsEventProcessor {
    /// The default HS endpoint this processor fetches events from
    pub homeserver: Homeserver,

    /// See [WatcherConfig::events_limit]
    pub limit: u16,
    pub files_path: PathBuf,
    pub event_handler: Arc<dyn EventHandler>,
    pub shutdown_rx: Receiver<bool>,
    /// Scheduler used to enqueue failed events onto the retry queue
    pub retry_scheduler: Arc<RetryScheduler>,
    /// PKDNS fallback for users with no `HOSTED_BY` mapping yet.
    pub user_hs_resolver: Arc<dyn PkdnsHomeserverResolver>,
}

/// A user's `HOSTED_BY` mapping as read by the event gate.
#[derive(Deserialize)]
struct UserHosting {
    homeserver_id: String,
    stale: bool,
}

#[async_trait::async_trait]
impl TEventProcessor for HsEventProcessor {
    fn files_path(&self) -> &PathBuf {
        &self.files_path
    }

    fn event_handler(&self) -> &Arc<dyn EventHandler> {
        &self.event_handler
    }

    fn instance_name(&self) -> String {
        format!("HsEventProcessor with HS ID: {}", self.homeserver.id)
    }

    fn retry_scheduler(&self) -> Option<&Arc<RetryScheduler>> {
        Some(&self.retry_scheduler)
    }

    fn homeserver_id(&self) -> Option<&str> {
        Some(self.homeserver.id.as_ref())
    }

    /// Refuses events for users no longer hosted here. Graph fast-path on `HOSTED_BY`;
    /// PKDNS fallback when no mapping exists.
    #[tracing::instrument(
        name = "event.gate",
        skip_all,
        fields(user = %event.parsed_uri.user_id(), hs = %self.homeserver.id)
    )]
    async fn should_process_event(&self, event: &Event) -> Result<bool, EventProcessorError> {
        let user_id = event.parsed_uri.user_id();

        let hosting: Option<UserHosting> =
            fetch_key_from_graph(queries::get::get_user_hosting(user_id), "hosting").await?;

        match hosting {
            Some(h) if h.homeserver_id == self.homeserver.id.as_ref() && !h.stale => Ok(true),
            // Diverges (elsewhere or stale): switching unsupported.
            Some(_) => {
                warn!("User mapping diverges from this homeserver; refusing event");
                Ok(false)
            }
            // No mapping yet: fall back to PKDNS.
            None => {
                let resolved = self
                    .user_hs_resolver
                    .resolve_homeserver(&user_id.to_public_key())
                    .await
                    .map_err(|e| EventProcessorError::client_error(e.to_string()))?;

                match resolved {
                    Some(ref resolved_hs) if *resolved_hs == self.homeserver.id => Ok(true),
                    // `None` here conflates "user has no published HS" with "DHT lookup flaked" —
                    // both refuse silently. Blocked on https://github.com/pubky/pubky-core/issues/408
                    // to distinguish them at the SDK boundary so flakes can be retried instead.
                    _ => {
                        warn!("User does not point to this homeserver in PKDNS; refusing event");
                        Ok(false)
                    }
                }
            }
        }
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
    /// - On error that should not be retried right now: stops the batch, cursor is not saved, next tick replays from same position
    /// - On MissingDependency: stores event in retry queue, continues processing
    /// - On 404 (blob not found): skips indexing, continues processing
    /// - On InvalidEventLine/SkipIndexing: logs and continues
    ///
    /// # Parameters
    /// - `lines`: A vector of strings representing event lines retrieved from the homeserver.
    #[tracing::instrument(name = "event_batch.process", skip_all, fields(batch.size = lines.len()))]
    pub async fn process_event_lines(&self, lines: Vec<String>) -> Result<(), EventProcessorError> {
        for line in &lines {
            if *self.shutdown_rx.borrow() {
                debug!(hs_id = %self.homeserver.id, "Shutdown detected; exiting event processing loop");
                return Ok(());
            }

            if let Some(cursor) = line.strip_prefix("cursor: ") {
                info!("Received cursor for the next request: {cursor}");
                match Homeserver::try_from_cursor(self.homeserver.id.clone(), cursor) {
                    Ok(hs) => hs.put_to_index().await?,
                    Err(e) => warn!("{e}"),
                }
                continue;
            }

            self.process_event_line(line).await?;
        }

        Ok(())
    }
}
