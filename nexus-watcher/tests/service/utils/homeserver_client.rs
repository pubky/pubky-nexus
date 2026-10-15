use std::collections::{HashMap, VecDeque};

use tokio::sync::Mutex;

use nexus_common::db::PubkyClientResult;
use nexus_common::models::user::PkdnsHomeserverResolver;
use nexus_watcher::homeserver::{
    HomeserverClient, HomeserverHealth, HsError, HsErrorKind, HsOp, HsStreamEvent,
};
use pubky::{PubkyResource, PublicKey};
use pubky_app_specs::PubkyId;

type UserEventsResult = Result<Vec<HsStreamEvent>, HsErrorKind>;

/// [`HomeserverClient`] that serves scripted answers, without a network.
#[derive(Default)]
pub struct MockHomeserverClient {
    /// Lines the first `poll_events` returns. Later polls return none.
    lines: Mutex<Vec<String>>,

    /// Homeserver IDs, cursors, and limits polled, in order.
    polls: Mutex<Vec<(String, u64, u16)>>,

    /// User event batches returned in fetch order.
    /// Useful when user ordering is not important and tests only care about processor flow.
    events: Mutex<VecDeque<UserEventsResult>>,

    /// User event batches returned by requested user ID.
    /// Useful when graph user ordering is intentionally not part of the assertion.
    user_events: Mutex<HashMap<String, UserEventsResult>>,

    /// User IDs, cursors, and limits requested from the mock, in fetch order.
    /// Useful for asserting the processor continued to, or stopped before, specific users.
    calls: Mutex<Vec<(String, u64, u16)>>,

    /// Bodies served by `get_resource`, by URI. Any other URI is a 404.
    resources: HashMap<String, Vec<u8>>,

    /// The homeserver every user resolves to. None by default.
    homeserver_of: Option<PubkyId>,

    health: HomeserverHealth,
}

impl MockHomeserverClient {
    pub fn with_lines(mut self, lines: Vec<String>) -> Self {
        *self.lines.get_mut() = lines;
        self
    }

    pub fn with_events(mut self, events: Vec<Vec<HsStreamEvent>>) -> Self {
        *self.events.get_mut() = events.into_iter().map(Ok).collect();
        self
    }

    pub fn with_results(mut self, results: Vec<UserEventsResult>) -> Self {
        *self.events.get_mut() = results.into();
        self
    }

    pub fn with_user_events(mut self, events: Vec<(String, Vec<HsStreamEvent>)>) -> Self {
        *self.user_events.get_mut() = events
            .into_iter()
            .map(|(user_id, events)| (user_id, Ok(events)))
            .collect();
        self
    }

    pub fn with_user_results(mut self, results: Vec<(String, UserEventsResult)>) -> Self {
        *self.user_events.get_mut() = results.into_iter().collect();
        self
    }

    pub fn with_homeserver_of(mut self, hs: PubkyId) -> Self {
        self.homeserver_of = Some(hs);
        self
    }

    pub fn with_resource(mut self, uri: impl Into<String>, body: impl Into<Vec<u8>>) -> Self {
        self.resources.insert(uri.into(), body.into());
        self
    }

    pub async fn polls(&self) -> Vec<(String, u64, u16)> {
        self.polls.lock().await.clone()
    }

    pub async fn calls(&self) -> Vec<String> {
        self.calls
            .lock()
            .await
            .iter()
            .map(|(user_id, _, _)| user_id.clone())
            .collect()
    }

    pub async fn call_details(&self) -> Vec<(String, u64, u16)> {
        self.calls.lock().await.clone()
    }
}

/// A 404 for `resource`, labelled with its owner's key, as the real client labels a failed
/// resource or blob fetch.
fn not_found(resource: &PubkyResource, op: HsOp) -> HsError {
    let owner = PubkyId::from(resource.owner.clone());
    HsError::new(&owner, op, HsErrorKind::NotFound(resource.to_pubky_url()))
}

#[async_trait::async_trait]
impl PkdnsHomeserverResolver for MockHomeserverClient {
    async fn resolve_homeserver(&self, _user_pk: &PublicKey) -> PubkyClientResult<Option<PubkyId>> {
        Ok(self.homeserver_of.clone())
    }
}

#[async_trait::async_trait]
impl HomeserverClient for MockHomeserverClient {
    async fn poll_events(
        &self,
        hs: &PubkyId,
        cursor: u64,
        limit: u16,
    ) -> Result<Vec<String>, HsError> {
        self.polls
            .lock()
            .await
            .push((hs.to_string(), cursor, limit));
        Ok(std::mem::take(&mut *self.lines.lock().await))
    }

    async fn user_events(
        &self,
        hs: &PubkyId,
        user: &PublicKey,
        cursor: u64,
        limit: u16,
    ) -> Result<Vec<HsStreamEvent>, HsError> {
        let user_id = user.z32();
        self.calls
            .lock()
            .await
            .push((user_id.clone(), cursor, limit));

        let result = match self.user_events.lock().await.remove(&user_id) {
            Some(result) => result,
            // Once the queued results are exhausted, further fetches resolve to an
            // empty success. Tests that model a persistently failing user must queue
            // one result per expected fetch (see the 404 backoff tests).
            None => self
                .events
                .lock()
                .await
                .pop_front()
                .unwrap_or_else(|| Ok(Vec::new())),
        };

        result.map_err(|kind| HsError::new(hs, HsOp::UserEvents, kind))
    }

    async fn get_resource(&self, resource: &PubkyResource) -> Result<Vec<u8>, HsError> {
        self.resources
            .get(&resource.to_pubky_url())
            .cloned()
            .ok_or_else(|| not_found(resource, HsOp::GetResource))
    }

    async fn get_blob(&self, resource: &PubkyResource) -> Result<Vec<u8>, HsError> {
        Err(not_found(resource, HsOp::GetBlob))
    }

    fn health(&self) -> &HomeserverHealth {
        &self.health
    }
}
