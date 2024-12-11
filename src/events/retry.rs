use dashmap::DashMap;
use log::{debug, info};
use std::{collections::LinkedList, sync::Arc};
use tokio::sync::{
    mpsc::{Receiver, Sender},
    Mutex,
};

use crate::types::PubkyId;

use super::{processor::EventErrorType, EventType};

pub const CHANNEL_BUFFER: usize = 1024;

#[derive(Debug, Clone)]
pub struct RetryEvent {
    pub uri: String,                   // URI of the resource
    pub event_type: EventType,         // Type of event (e.g., PUT, DEL)
    pub timestamp: u64,                // Unix timestamp when the event was received
    pub dependency: Option<String>,    // Optional parent URI for dependency tracking
    pub retry_count: u32,              // Number of retries attempted
    pub error_type: EventErrorType, // Optional field to track failure reasons
}

impl RetryEvent {
    pub fn new(
        uri: &String,
        event_type: &EventType,
        dependency: Option<String>,
        error_type: EventErrorType
    ) -> Self {
        Self {
            uri: uri.to_string(),
            event_type: event_type.clone(),
            // TODO: Add now unixtime
            timestamp: 1733831902,
            dependency,
            retry_count: 0,
            error_type,
        }
    }
}

#[derive(Debug, Clone)]
pub enum SenderMessage {
    Retry(String),           // Retry events associated with this key
    Add(PubkyId, RetryEvent), // Add a new RetryEvent to the fail_events
}

pub type SenderChannel = Arc<Mutex<Sender<SenderMessage>>>;
type ReceiverChannel = Arc<Mutex<Receiver<SenderMessage>>>;

pub struct RetryManager {
    pub sender: SenderChannel,
    receiver: ReceiverChannel,
    fail_events: DashMap<String, LinkedList<RetryEvent>>,
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
            fail_events: DashMap::new(),
        }
    }

    pub async fn exec(&self) {
        let mut rx = self.receiver.lock().await;
        while let Some(message) = rx.recv().await {
            match message {
                SenderMessage::Retry(homeserver_pubky) => {
                    self.retry_events_for_homeserver(&homeserver_pubky).await;
                }
                SenderMessage::Add(homeserver_pubky, retry_event) => {
                    self.add_fail_event(homeserver_pubky, retry_event);
                }
            }
        }
    }

    async fn retry_events_for_homeserver(&self, homeserver_pubky: &str) {
        if let Some(retry_events) = self.fail_events.get(homeserver_pubky) {
            info!(
                "** RETRY_MANAGER ===> Trying to fetch the failing events from {:?}",
                homeserver_pubky
            );
            for event in retry_events.iter() {
                info!("-> {:?}:{}:{:?}", event.event_type, event.uri, event.error_type);
            }
        } else {
            info!("No retry events found for key: {}", homeserver_pubky);
        }
    }

    fn add_fail_event(&self, homeserver_pubky: PubkyId, retry_event: RetryEvent) {
        let mut list = self
            .fail_events
            .entry(homeserver_pubky.to_string())
            .or_insert_with(LinkedList::new);
        list.push_back(retry_event);
        debug!(
            "** RETRY_MANAGER:  Added fail event for homeserver_pubky: {}",
            homeserver_pubky
        );
    }
}
