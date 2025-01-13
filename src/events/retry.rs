use chrono::Utc;
use log::info;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{
    mpsc::{Receiver, Sender},
    Mutex,
};

use crate::{
    types::{DynError, PubkyId},
    RedisOps,
};

use super::{processor::EventErrorType, EventType};

pub const CHANNEL_BUFFER: usize = 1024;
pub const RETRY_MAMAGER_PREFIX: &str = "RetryManager";
pub const RETRY_MANAGER_EVENTS_INDEX: &str = "events";
pub const RETRY_MANAGER_STATE_INDEX: &str = "state";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryEvent {
    pub uri: String,                     // URI of the resource
    pub event_type: EventType,           // Type of event (e.g., PUT, DEL)
    pub timestamp: i64,                  // Unix timestamp when the event was received
    pub dependency: Option<Vec<String>>, // Optional parent URI for dependency tracking
    pub retry_count: u32,                // Number of retries attempted
    pub error_type: EventErrorType,      // Optional field to track failure reasons
}

impl RedisOps for RetryEvent {}

impl RetryEvent {
    pub fn new(
        uri: &String,
        event_type: &EventType,
        dependency: Option<Vec<String>>,
        error_type: EventErrorType,
    ) -> Self {
        Self {
            uri: uri.to_string(),
            event_type: event_type.clone(),
            timestamp: Utc::now().timestamp_millis(),
            dependency,
            retry_count: 0,
            error_type,
        }
    }

    fn get_events_index(homeserver: &str) -> [&str; 2] {
        [homeserver, RETRY_MANAGER_EVENTS_INDEX]
    }

    fn get_state_index(homeserver: &str) -> [&str; 2] {
        [homeserver, RETRY_MANAGER_STATE_INDEX]
    }

    pub async fn put_to_index(&self, homeserver: &str) -> Result<(), DynError> {
        Self::put_index_sorted_set(
            &Self::get_events_index(homeserver),
            &[(self.timestamp as f64, &self.uri)],
            Some(RETRY_MAMAGER_PREFIX),
            None,
        )
        .await?;

        let event_serialized = serde_json::to_string(self)?;

        Self::put_index_hash_map(
            Some(RETRY_MAMAGER_PREFIX),
            &Self::get_state_index(homeserver),
            &self.uri,
            event_serialized,
        )
        .await?;
        Ok(())
    }

    pub async fn check_uri(homeserver: &str, pubky_uri: &str) -> Result<Option<isize>, DynError> {
        if let Some(post_details) = Self::check_sorted_set_member(
            Some(RETRY_MAMAGER_PREFIX),
            &Self::get_events_index(homeserver),
            &[pubky_uri],
        )
        .await?
        {
            return Ok(Some(post_details));
        }
        Ok(None)
    }

    pub async fn get_from_hash_map_index(
        homeserver: &str,
        pubky_uri: &str,
    ) -> Result<Option<Self>, DynError> {
        let mut found_event = None;
        if let Some(event_state) = Self::get_index_hash_map(
            Some(RETRY_MAMAGER_PREFIX),
            &Self::get_state_index(homeserver),
            pubky_uri,
        )
        .await?
        {
            let event = serde_json::from_str::<Self>(&event_state)?;
            found_event = Some(event);
        }
        Ok(found_event)
    }
}

#[derive(Debug, Clone)]
pub enum SenderMessage {
    Retry(String),            // Retry events associated with this key
    Add(PubkyId, RetryEvent), // Add a new RetryEvent to the fail_events
}

pub type SenderChannel = Arc<Mutex<Sender<SenderMessage>>>;
type ReceiverChannel = Arc<Mutex<Receiver<SenderMessage>>>;

pub struct RetryManager {
    pub sender: SenderChannel,
    receiver: ReceiverChannel,
}

/// Initializes a new `RetryManager` with a message-passing channel.
///
/// This function sets up the `RetryManager` by taking a tuple containing
/// a `Sender` and `Receiver` from a multi-producer, single-consumer (mpsc) channel.
///
/// # Parameters
/// - `(tx, rx)`: A tuple containing:
///   - `tx` (`Sender<String>`): The sending half of the mpsc channel, used to dispatch messages to the manager.
///   - `rx` (`Receiver<String>`): The receiving half of the mpsc channel, used to listen for incoming messages.
impl RetryManager {
    pub fn initialise((tx, rx): (Sender<SenderMessage>, Receiver<SenderMessage>)) -> Self {
        Self {
            receiver: Arc::new(Mutex::new(rx)),
            sender: Arc::new(Mutex::new(tx)),
        }
    }

    pub async fn exec(&self) -> Result<(), DynError> {
        let mut rx = self.receiver.lock().await;
        // Listen all the messages in the channel
        while let Some(message) = rx.recv().await {
            match message {
                SenderMessage::Retry(homeserver_pubky) => {
                    self.retry_events_for_homeserver(&homeserver_pubky);
                }
                SenderMessage::Add(homeserver_pubky, retry_event) => {
                    self.add_fail_event(homeserver_pubky, retry_event).await?;
                }
            }
        }
        Ok(())
    }

    fn retry_events_for_homeserver(&self, _homeserver_pubky: &str) {
        // WIP: Retrieve the homeserver events from the SORTED SET
        // RetryManager:homeserver_pubky:events
    }

    async fn add_fail_event(
        &self,
        homeserver_pubky: PubkyId,
        retry_event: RetryEvent,
    ) -> Result<(), DynError> {
        info!(
            "Add fail event to Redis: {:?}: {}",
            retry_event.event_type, retry_event.uri
        );
        // Write in the index
        retry_event.put_to_index(&homeserver_pubky).await?;
        Ok(())
    }
}
