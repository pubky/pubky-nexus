use super::TEventProcessor;
use crate::events::retry::RetryScheduler;
use crate::events::EventHandler;
use nexus_common::db::{fetch_row_from_graph, queries, GraphResult, PubkyConnector};
use nexus_common::models::event::{Event, EventProcessorError};
use nexus_common::models::homeserver::Homeserver;
use pubky::Method;
use pubky_app_specs::PubkyId;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch::Receiver;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// A user's `HOSTED_BY` mapping, classified relative to a processor's HS.
///
/// The `stale` flag is only carried where it is meaningful: a stale mapping means
/// the user's published HS has diverged from the stored one (see
/// [`set_user_homeserver_stale`](nexus_common::db::queries::put::set_user_homeserver_stale)).
#[derive(Clone)]
pub enum HsMapping {
    /// The user has no `HOSTED_BY` edge yet.
    Unbound,
    /// The user is mapped to this processor's HS.
    Current { stale: bool },
    /// The user is mapped to a different HS.
    Other { hs_id: String },
}

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

    /// Per-run cache of users' `HOSTED_BY` mappings. For a given user's events in
    /// the events list, only the 1st one results in a graph lookup, the rest read from this cache.
    ///
    /// Entries are deliberately never refreshed within a run: once a user's mapping
    /// is resolved, the same decision is reused for the rest of the batch even if the
    /// resolver realigns the underlying edge mid-run. The cache is dropped when the run ends.
    pub hs_mapping_cache: Mutex<HashMap<String, HsMapping>>,
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

    /// Skips events from users that are not actively bound to this homeserver.
    ///
    /// Before an event is processed we inspect the user's `HOSTED_BY` edge:
    /// - No edge, or a non-stale edge to this processor's homeserver: process.
    /// - A stale edge to this homeserver (the user's published homeserver has
    ///   diverged): log a warning and skip until the resolver realigns it.
    /// - An edge to a different homeserver: log a warning and skip.
    ///
    /// A lookup failure fails open (the event is processed), so a transient
    /// graph error never silently drops legitimate events.
    async fn should_process_event(&self, event: &Event) -> bool {
        let user_id = event.parsed_uri.user_id();

        match self.user_hs_mapping(user_id).await {
            // No mapping yet (graceful fallback) or actively bound here: process.
            Ok(HsMapping::Unbound | HsMapping::Current { stale: false }) => true,

            // Bound here but the mapping is stale: skip until the resolver realigns it.
            Ok(HsMapping::Current { stale: true }) => {
                warn!(
                    event.uri = %event.uri,
                    user_id = %user_id,
                    processor_homeserver = %self.homeserver.id,
                    "User's homeserver mapping is stale; skipping event"
                );
                false
            }

            // Bound to a different homeserver: skip.
            Ok(HsMapping::Other { hs_id }) => {
                warn!(
                    event.uri = %event.uri,
                    user_id = %user_id,
                    processor_homeserver = %self.homeserver.id,
                    user_homeserver = %hs_id,
                    "User is hosted on a different homeserver; skipping event"
                );
                false
            }

            // Lookup failed: fail open so transient graph errors don't drop events.
            Err(e) => {
                warn!(
                    event.uri = %event.uri,
                    user_id = %user_id,
                    "Failed to resolve user's homeserver; processing event anyway: {e}"
                );
                true
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
    /// Resolves and caches a user's `HOSTED_BY` mapping relative to this processor's HS
    async fn user_hs_mapping(&self, user_id: &PubkyId) -> GraphResult<HsMapping> {
        if let Some(hs_mapping) = self.hs_mapping_cache.lock().await.get(user_id.as_ref()) {
            return Ok(hs_mapping.clone());
        }

        let query = queries::get::get_user_homeserver(user_id.as_ref());
        let mapping = match fetch_row_from_graph(query).await? {
            None => HsMapping::Unbound,
            Some(row) => {
                let hs_id: String = row.get("homeserver_id")?;
                let stale: bool = row.get("stale")?;

                if hs_id.as_str() == self.homeserver.id.as_ref() {
                    HsMapping::Current { stale }
                } else {
                    HsMapping::Other { hs_id }
                }
            }
        };

        self.hs_mapping_cache
            .lock()
            .await
            .insert(user_id.as_ref().to_string(), mapping.clone());

        Ok(mapping)
    }

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
