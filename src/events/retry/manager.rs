use log::info;
use std::sync::Arc;
use tokio::sync::{
    mpsc::{Receiver, Sender},
    Mutex,
};

use crate::types::DynError;

use super::event::RetryEvent;

pub const CHANNEL_BUFFER: usize = 1024;

#[derive(Debug, Clone)]
pub enum SenderMessage {
    // Command to retry all pending events that are waiting to be indexed
    Retry(String),
    // Command to add a failed event to the RetryManager cache for future processing
    Add(String, RetryEvent),
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

    /// Executes the main event loop to process messages from the channel
    /// This function listens for incoming messages on the receiver channel and handles them
    /// based on their type:
    /// - **`SenderMessage::Retry`**: Triggers the retry process
    /// - **`SenderMessage::Add`**: Queues a failed event in the retry cache for future processing
    ///
    /// The loop runs continuously until the channel is closed, ensuring that all messages
    /// are processed appropriately.
    pub async fn exec(&self) -> Result<(), DynError> {
        let mut rx = self.receiver.lock().await;
        // Listen all the messages in the channel
        while let Some(message) = rx.recv().await {
            match message {
                SenderMessage::Retry(homeserver_pubky) => {
                    self.retry_events_for_homeserver(&homeserver_pubky);
                }
                SenderMessage::Add(index_key, retry_event) => {
                    self.queue_failed_event(index_key, retry_event).await?;
                }
            }
        }
        Ok(())
    }

    fn retry_events_for_homeserver(&self, _homeserver_pubky: &str) {
        // WIP: Retrieve the homeserver events from the SORTED SET
        // RetryManager:events
    }

    /// Stores the event line in the Redis cache, adding it to the retry queue for
    /// future processing
    /// # Arguments
    /// - `index_key`: A `String` representing the compacted key for the event to be stored in Redis
    /// - `retry_event`: A `RetryEvent` instance containing the details of the failed event
    async fn queue_failed_event(
        &self,
        index_key: String,
        retry_event: RetryEvent,
    ) -> Result<(), DynError> {
        info!("Add fail event to Redis: {}", index_key);
        // Write in the index
        retry_event.put_to_index(index_key).await?;
        Ok(())
    }
}
