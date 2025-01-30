use log::{debug, error};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::{
    mpsc::{Receiver, Sender},
    Mutex,
};

use super::event::RetryEvent;

pub const CHANNEL_BUFFER: usize = 1024;

/// Represents commands sent to the retry manager for handling events in the retry queue
#[derive(Debug, Clone)]
pub enum RetryQueueMessage {
    // Command to retry all pending events that are waiting to be indexed
    RetryEvent(String),
    /// Command to process a specific event in the retry queue
    /// This action can either add an event to the queue or use the event as context to remove it from the queue
    /// - `String`: The index or identifier of the event being processed
    /// - `RetryEvent`: The event to be added to or processed for removal from the retry queue
    ProcessEvent(String, RetryEvent),
}

pub type RetryManagerSenderChannel = Arc<Mutex<Sender<RetryQueueMessage>>>;
//type RetryManagerReceiverChannel = Receiver<RetryQueueMessage>;

/// Manages the retry queue for processing failed events.
///
/// The `RetryManager` is responsible for handling messages that involve retrying or
/// processing failed events in an asynchronous event system. It listens for incoming
/// retry-related messages and processes them accordingly
///
/// ## Responsibilities:
/// - Receives and processes messages related to event retries
/// - Maintains a queue of events that need to be retried
/// - Ensures failed events are reprocessed in an orderly manner
pub struct RetryManager {}

/// Initializes a new `RetryManager` with a message-passing channel
impl RetryManager {
    pub fn init_channels() -> (Receiver<RetryQueueMessage>, RetryManagerSenderChannel) {
        let (tx, rx) = mpsc::channel(CHANNEL_BUFFER);
        (
            rx,                       // Receiver channel
            Arc::new(Mutex::new(tx)), // Sender channel
        )
    }

    /// Executes the main event loop to process messages from the channel
    /// This function listens for incoming messages on the receiver channel and handles them
    /// based on their type
    /// The loop runs continuously until the channel is closed, ensuring that all messages
    /// are processed appropriately
    ///
    ///  # Arguments
    /// * `rx` - The receiver channel for retry messages
    pub async fn process_messages(mut rx: Receiver<RetryQueueMessage>) {
        tokio::spawn(async move {
            // Listen all the messages in the channel
            while let Some(message) = rx.recv().await {
                match message {
                    RetryQueueMessage::ProcessEvent(index_key, retry_event) => {
                        error!("{}, {}", retry_event.error_type, index_key);
                        if let Err(err) = retry_event.put_to_index(index_key).await {
                            error!("Failed to put event to index: {}", err);
                        }
                    }
                    _ => debug!("New message received, not handled: {:?}", message),
                }
            }
        });
    }
}
